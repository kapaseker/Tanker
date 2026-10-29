//! 验证命令解析、产物预检查与超时监视入口。

use super::{TIMEOUT, capture::write_new, offscreen::spawn_watchdog, scenarios::Scenario};
use bevy::prelude::*;
use std::{ffi::OsString, fs, path::PathBuf, sync::mpsc};

/// 命令行指定的输出目标；单张截图不生成额外报告。
#[derive(Debug, PartialEq, Eq, Resource, Clone)]
pub(crate) struct Options {
    pub(super) scenario: Scenario,
    pub(super) output: PathBuf,
    pub(super) single_image: bool,
    /// 仅 wheels 接受 --model enemy；默认继续验证玩家。
    pub(super) enemy_wheels: bool,
}

impl Options {
    /// 无参数返回普通游戏；其余参数必须完整匹配已公布的接口。
    pub(crate) fn parse(args: Vec<OsString>) -> Result<Option<Self>, String> {
        let usage = "用法：--capture <图片.png> 或 --offscreen-test <scene|drive|combat|wheels|index|gallery|detail|hud> --output <目录> [--model enemy]（仅 wheels）";
        if args.is_empty() {
            return Ok(None);
        }
        let options = if args.len() == 2 && args[0] == "--capture" {
            let output = PathBuf::from(&args[1]);
            if output
                .extension()
                .is_none_or(|ext| !ext.eq_ignore_ascii_case("png"))
            {
                return Err(format!("截图必须使用 .png 扩展名。{usage}"));
            }
            Self {
                scenario: Scenario::Scene,
                output,
                single_image: true,
                enemy_wheels: false,
            }
        } else if (args.len() == 4 || args.len() == 6)
            && args[0] == "--offscreen-test"
            && args[2] == "--output"
        {
            let scenario = match args[1].to_str() {
                Some("scene") => Scenario::Scene,
                Some("drive") => Scenario::Drive,
                Some("combat") => Scenario::Combat,
                Some("wheels") => Scenario::Wheels,
                Some("index") => Scenario::Index,
                Some("gallery") => Scenario::Gallery,
                Some("detail") => Scenario::Detail,
                Some("hud") => Scenario::Hud,
                _ => return Err(usage.into()),
            };
            let enemy_wheels = args.len() == 6;
            if enemy_wheels
                && (scenario != Scenario::Wheels || args[4] != "--model" || args[5] != "enemy")
            {
                return Err(usage.into());
            }
            Self {
                scenario,
                output: PathBuf::from(&args[3]),
                single_image: false,
                enemy_wheels,
            }
        } else {
            return Err(usage.into());
        };
        if options.output.as_os_str().is_empty() {
            return Err("输出路径不能为空".into());
        }
        Ok(Some(options))
    }

    /// 将所有目标提前检查一遍；实际写入仍用 create_new 防止并发覆盖。
    pub(crate) fn prepare(&self) -> Result<(), String> {
        for path in self.paths() {
            if fs::symlink_metadata(&path).is_ok() {
                return Err(format!("目标已存在，拒绝覆盖：{}", path.display()));
            }
            if let Some(parent) = path.parent().filter(|path| !path.as_os_str().is_empty()) {
                fs::create_dir_all(parent).map_err(|e| format!("无法创建输出目录：{e}"))?;
            }
        }
        Ok(())
    }

    /// 启动即计时；独立监视线程也能处理 GPU 驱动或同步管线编译阻塞。
    pub(crate) fn watchdog(&self) -> mpsc::Sender<()> {
        let report = (!self.single_image).then(|| self.output.join("report.txt"));
        spawn_watchdog(TIMEOUT, move || {
            let message = "离屏验证超过 120 秒，强制结束；结果：失败";
            eprintln!("{message}");
            if let Some(path) = report {
                let _ = write_new(&path, message.as_bytes());
            }
            std::process::exit(1);
        })
    }

    /// 单张与批量截图使用相同关键帧流程，只改变产物路径。
    pub(super) fn image_path(&self, frame: u32) -> PathBuf {
        if self.single_image {
            self.output.clone()
        } else {
            self.output.join(format!("frame-{frame:06}.png"))
        }
    }

    /// 返回本次运行会写入的全部产物，供启动前拒绝覆盖。
    fn paths(&self) -> Vec<PathBuf> {
        let mut paths: Vec<_> = self
            .scenario
            .frames()
            .iter()
            .map(|&frame| self.image_path(frame))
            .collect();
        if !self.single_image {
            paths.push(self.output.join("report.txt"));
            if self.scenario == Scenario::Scene {
                paths.push(self.output.join("concrete.png"));
            }
            if self.scenario == Scenario::Wheels {
                for name in [
                    "side",
                    "front",
                    "rear",
                    "bounds",
                    "comparison",
                    "front-on",
                    "rear-on",
                    "front-above",
                    "grille",
                ] {
                    paths.push(self.output.join(format!("{name}.png")));
                }
            }
        }
        paths
    }
}

#[cfg(test)]
mod tests {
    use super::super::capture::create_new;
    use super::*;
    fn parse(args: &[&str]) -> Result<Option<Options>, String> {
        Options::parse(args.iter().map(Into::into).collect())
    }

    #[test]
    fn command_line_keeps_normal_game_and_accepts_documented_commands() {
        assert_eq!(parse(&[]).unwrap(), None);
        let capture = parse(&["--capture", "截图.png"]).unwrap().unwrap();
        let enemy = parse(&[
            "--offscreen-test",
            "wheels",
            "--output",
            "out",
            "--model",
            "enemy",
        ])
        .unwrap()
        .unwrap();
        assert!(enemy.enemy_wheels);
        assert!(capture.single_image);
        assert_eq!(capture.scenario, Scenario::Scene);
        for (name, scenario) in [
            ("scene", Scenario::Scene),
            ("drive", Scenario::Drive),
            ("combat", Scenario::Combat),
            ("wheels", Scenario::Wheels),
            ("index", Scenario::Index),
            ("gallery", Scenario::Gallery),
            ("detail", Scenario::Detail),
            ("hud", Scenario::Hud),
        ] {
            let options = parse(&["--offscreen-test", name, "--output", "out"])
                .unwrap()
                .unwrap();
            assert_eq!(options.scenario, scenario);
            assert!(!options.single_image);
        }
    }

    #[test]
    fn invalid_arguments_fail_before_starting_renderer() {
        for args in [
            vec!["--capture"],
            vec!["--capture", "a.jpg"],
            vec!["--capture", ""],
            vec!["--offscreen-test", "unknown", "--output", "out"],
            vec!["--offscreen-test", "scene"],
            vec!["--unknown"],
            vec![
                "--offscreen-test",
                "scene",
                "--output",
                "out",
                "--model",
                "enemy",
            ],
            vec![
                "--offscreen-test",
                "wheels",
                "--output",
                "out",
                "--model",
                "unknown",
            ],
            vec!["--capture", "a.png", "extra"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
    }

    #[test]
    fn output_creation_refuses_existing_files_and_reports_io_failures() {
        let directory = std::env::temp_dir().join(format!(
            "tanker-offscreen-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = directory.join("nested/capture.png");
        let options = Options {
            scenario: Scenario::Scene,
            output: path.clone(),
            single_image: true,
            enemy_wheels: false,
        };
        options.prepare().unwrap();
        write_new(&path, b"existing evidence").unwrap();
        assert!(options.prepare().is_err());
        assert!(write_new(&path, b"overwrite").is_err());
        assert_eq!(fs::read(&path).unwrap(), b"existing evidence");
        assert!(create_new(&directory).is_err());
        fs::remove_file(path).unwrap();
        fs::remove_dir(directory.join("nested")).unwrap();
        fs::remove_dir(directory).unwrap();
    }
}
