//! 菜单背景音乐：启动时发现曲目，随页面过渡暂停和续播。

use super::{
    state::GameState,
    transition::{self, Fade, Phase},
};
use bevy::{
    audio::{AudioSinkPlayback, Decodable, Volume},
    prelude::*,
};
use std::path::{Path, PathBuf};

/// 背景音乐相对于资源根目录的位置。
const MUSIC_DIR: &str = "music/background";
/// 背景音乐的线性音量。
const MUSIC_VOLUME: f32 = 0.5;

/// 已验证的曲目及下一首的位置。
#[derive(Resource, Default)]
struct Playlist {
    tracks: Vec<Handle<AudioSource>>,
    next: usize,
    resume: bool,
}

/// 标记唯一的背景音乐播放实体。
#[derive(Component)]
struct BackgroundMusic;

/// 安装背景音乐发现及播放系统。
pub(super) struct MusicPlugin;

impl Plugin for MusicPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Playlist>()
            .add_systems(Startup, load_tracks)
            .add_systems(Update, play_music.after(transition::advance));
    }
}

/// 按文件名排序，仅发现目录本身的 MP3 文件。
fn track_paths(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut tracks = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && entry
                .path()
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("mp3"))
        {
            tracks.push(entry.path());
        }
    }
    tracks.sort();
    Ok(tracks)
}

/// 载入曲目并预检解码器，避免 Bevy 对无效音频的解码 panic 中断游戏。
fn load_tracks(mut playlist: ResMut<Playlist>, mut audio: ResMut<Assets<AudioSource>>) {
    let dir = bevy::asset::io::file::FileAssetReader::get_base_path()
        .join("assets")
        .join(MUSIC_DIR);
    let paths = match track_paths(&dir) {
        Ok(paths) => paths,
        Err(error) => {
            warn!("无法读取背景音乐目录 {}：{error}", dir.display());
            return;
        }
    };
    for path in paths {
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) => {
                warn!("无法读取背景音乐 {}：{error}", path.display());
                continue;
            }
        };
        let source = AudioSource {
            bytes: bytes.into(),
        };
        if std::panic::catch_unwind(|| source.decoder()).is_err() {
            warn!("背景音乐格式无效，已跳过：{}", path.display());
            continue;
        }
        playlist.tracks.push(audio.add(source));
    }
    if playlist.tracks.is_empty() {
        warn!("没有可播放的背景音乐：{}", dir.display());
    }
}

/// 返回当前音乐音量；None 表示保持静音并暂停。
fn music_volume(state: GameState, fade: &Fade, resume: bool) -> Option<f32> {
    match state {
        GameState::ToGame | GameState::ToTestScene | GameState::ToExit => {
            (fade.alpha < 1.0).then_some(MUSIC_VOLUME * (1.0 - fade.alpha))
        }
        GameState::InGame | GameState::TestScene | GameState::ToIndex => None,
        GameState::Index if resume && fade.phase == Phase::Waiting => None,
        GameState::Index if resume && fade.phase == Phase::In => {
            Some(MUSIC_VOLUME * (1.0 - fade.alpha))
        }
        _ => Some(MUSIC_VOLUME),
    }
}

/// 播放下一首，并以画面过渡的透明度控制播放和暂停。
fn play_music(
    mut commands: Commands,
    mut playlist: ResMut<Playlist>,
    state: Res<State<GameState>>,
    fade: Res<Fade>,
    mut player: Query<(Entity, Option<&mut AudioSink>), With<BackgroundMusic>>,
) {
    let volume = music_volume(*state.get(), &fade, playlist.resume);
    if volume.is_none() {
        playlist.resume = true;
    } else if *state.get() == GameState::Index && fade.phase == Phase::Idle {
        playlist.resume = false;
    }
    if let Some((entity, sink)) = player.iter_mut().next() {
        if let Some(mut sink) = sink {
            if sink.empty() {
                commands.entity(entity).despawn();
            } else if let Some(volume) = volume {
                sink.set_volume(Volume::Linear(volume));
                sink.play();
            } else {
                sink.pause();
            }
        }
        return;
    }
    if let Some(volume) = volume.filter(|_| !playlist.tracks.is_empty()) {
        let track = playlist.tracks[playlist.next].clone();
        playlist.next = (playlist.next + 1) % playlist.tracks.len();
        commands.spawn((
            BackgroundMusic,
            AudioPlayer::new(track),
            PlaybackSettings::ONCE.with_volume(Volume::Linear(volume)),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn music_follows_game_and_return_fades() {
        let mut fade = Fade::default();
        fade.alpha = 0.5;
        assert_eq!(music_volume(GameState::ToGame, &fade, false), Some(0.25));
        assert_eq!(music_volume(GameState::InGame, &fade, false), None);
        assert_eq!(music_volume(GameState::TestScene, &fade, false), None);
        assert_eq!(music_volume(GameState::ToIndex, &fade, true), None);
        fade.phase = Phase::Waiting;
        assert_eq!(music_volume(GameState::Index, &fade, true), None);
        fade.phase = Phase::In;
        fade.target = GameState::Index;
        assert_eq!(music_volume(GameState::Index, &fade, true), Some(0.25));
        assert_eq!(music_volume(GameState::Index, &fade, false), Some(0.5));
        assert_eq!(music_volume(GameState::Gallery, &fade, false), Some(0.5));
    }

    #[test]
    fn finds_only_sorted_mp3_files() {
        let dir = std::env::temp_dir().join(format!("tanker-music-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("b.MP3"), []).unwrap();
        std::fs::write(dir.join("a.mp3"), []).unwrap();
        std::fs::write(dir.join("c.txt"), []).unwrap();
        let paths = track_paths(&dir).unwrap();
        assert_eq!(paths, vec![dir.join("a.mp3"), dir.join("b.MP3")]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn advances_and_wraps_playlist() {
        let mut audio = Assets::<AudioSource>::default();
        let first = audio.add(AudioSource {
            bytes: vec![1].into(),
        });
        let second = audio.add(AudioSource {
            bytes: vec![2].into(),
        });
        let mut app = App::new();
        app.insert_resource(Playlist {
            tracks: vec![first.clone(), second.clone()],
            next: 0,
            resume: false,
        })
        .insert_resource(State::new(GameState::Index))
        .insert_resource(Fade::default())
        .add_systems(Update, play_music);
        for expected in [first.clone(), second, first] {
            app.update();
            let world = app.world_mut();
            let (entity, player) = world
                .query_filtered::<(Entity, &AudioPlayer), With<BackgroundMusic>>()
                .single(world)
                .unwrap();
            assert_eq!(player.0, expected);
            world.despawn(entity);
        }
    }
}
