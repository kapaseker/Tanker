// 独立 HTML 共用的切页控制：正式导航前可取消，固定标题栏与角标不参与遮罩。
(() => {
  const root = document.documentElement;
  const duration = 600;
  const markerKey = 'tanker-page-transition';
  root.dataset.transition = 'boot';

  document.addEventListener('DOMContentLoaded', () => {
    const bar = document.querySelector('.backbar');
    const back = bar.querySelector('.back');
    const title = bar.querySelector('.backbar__title');
    const sourceTitle = title.textContent;
    const sourceLabel = back.getAttribute('aria-label');
    const home = back.classList.contains('back--home');
    const overlay = document.createElement('div');
    overlay.className = 'page-fade';
    overlay.setAttribute('aria-hidden', 'true');
    document.body.append(overlay);
    let animation;
    let savedFocus;
    let destination;
    let marker;
    // file:// 或隐私模式可能禁止存储；动画仍可独立工作。
    try {
      marker = JSON.parse(sessionStorage.getItem(markerKey));
      sessionStorage.removeItem(markerKey);
    } catch { /* 无存储时仅省略跨文档焦点恢复。 */ }

    function restore() {
      animation?.cancel();
      overlay.style.opacity = '0';
      root.dataset.transition = 'idle';
      title.textContent = sourceTitle;
      back.setAttribute('aria-label', sourceLabel);
    }

    // 从屏幕当前亮度继续，淡入途中返回也不会跳亮。
    function fade(to, finished) {
      const from = Number(getComputedStyle(overlay).opacity);
      animation?.cancel();
      overlay.style.opacity = String(to);
      animation = overlay.animate([{ opacity: from }, { opacity: to }], {
        duration: duration * Math.abs(to - from), easing: 'linear', fill: 'forwards',
      });
      animation.onfinish = finished;
    }

    function cancel() {
      restore();
      savedFocus?.focus({ preventScroll: true });
    }

    function navigate(link) {
      if (['out', 'waiting'].includes(root.dataset.transition)) return;
      const url = new URL(link.href);
      const name = url.pathname.split('/').pop();
      const titles = { 'index.html': '主菜单', 'model-gallery.html': '模型画廊', 'model-detail.html': '模型详情' };
      if (!titles[name] || url.origin !== location.origin) return;
      savedFocus = document.activeElement;
      destination = url.href;
      title.textContent = titles[name];
      back.setAttribute('aria-label', '取消页面切换');
      // 先读取当前遮罩亮度，再改变阶段，防止 CSS 将透明度提前设为全黑。
      fade(1, () => {
        // 提交导航后旧文档可能继续显示，但已越过可取消边界。
        root.dataset.transition = 'waiting';
        try {
          sessionStorage.setItem(markerKey, JSON.stringify({
            target: destination, source: location.href,
          }));
        } catch { /* 导航不依赖存储成功。 */ }
        location.assign(destination);
      });
      root.dataset.transition = 'out';
    }

    document.addEventListener('click', (event) => {
      const link = event.target.closest('a');
      const isBack = link === back;
      if (root.dataset.transition === 'waiting') {
        event.preventDefault();
        event.stopImmediatePropagation();
        return;
      }
      if (root.dataset.transition === 'out') {
        event.preventDefault();
        event.stopImmediatePropagation();
        if (isBack) cancel();
        return;
      }
      if (root.dataset.transition !== 'idle' && !isBack) {
        event.preventDefault();
        event.stopImmediatePropagation();
        return;
      }
      if (!link || event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey || link.target || link.hasAttribute('download')) return;
      const url = new URL(link.href);
      if (url.pathname === location.pathname && url.search === location.search && !isBack) return;
      if (!['index.html', 'model-gallery.html', 'model-detail.html'].includes(url.pathname.split('/').pop()) || url.origin !== location.origin) return;
      event.preventDefault();
      event.stopImmediatePropagation();
      if (!home || !isBack) navigate(link);
    }, true);

    document.addEventListener('keydown', (event) => {
      const phase = root.dataset.transition;
      if (phase === 'waiting') {
        event.preventDefault();
        event.stopImmediatePropagation();
        return;
      }
      if (event.key === 'Escape') {
        event.preventDefault();
        event.stopImmediatePropagation();
        if (event.repeat) return;
        if (phase === 'out') cancel();
        else if (!home) navigate(back);
      } else if (phase !== 'idle') {
        event.preventDefault();
        event.stopImmediatePropagation();
        if (event.repeat) return;
        if (event.key === 'Tab' && (!home || phase === 'out')) back.focus({ preventScroll: true });
        if (event.key === 'Enter' && document.activeElement === back) {
          if (phase === 'out') cancel();
          else if (!home) navigate(back);
        }
      }
    }, true);

    // 不用 inert：取消时保留正文焦点，返回框的鼠标按下也不抢走它。
    for (const type of ['pointerdown', 'pointermove', 'pointerup', 'wheel']) {
      document.addEventListener(type, (event) => {
        if (root.dataset.transition === 'idle') return;
        if (event.target.closest('.backbar') && type !== 'wheel') {
          if (type === 'pointerdown' && root.dataset.transition === 'out') event.preventDefault();
          return;
        }
        event.preventDefault();
        event.stopImmediatePropagation();
      }, { capture: true, passive: false });
    }

    root.dataset.transition = 'in';
    // 等首帧与字体加载完再淡入，避免文字在动画中替换。
    document.fonts.ready.then(() => requestAnimationFrame(() => requestAnimationFrame(() => {
      if (root.dataset.transition !== 'in') return;
      fade(0, () => {
        restore();
        if (marker?.target === location.href && home && new URL(marker.source).pathname.endsWith('/model-gallery.html')) {
          const item = document.querySelector('.menu__item[href="model-gallery.html"]');
          item.focus({ preventScroll: true });
          // 鼠标一旦进入菜单就撤销恢复的焦点，
          // 否则 :focus-visible 高亮残留，与另一项的 :hover 叠加成双选中；键盘用户不受影响。
          item.closest('.menu')?.addEventListener('pointerover', () => item.blur(), { once: true });
        }
      });
    })));

    // 原生后退使用浏览器缓存时，不保留离开页面时的全黑遮罩。
    window.addEventListener('pageshow', (event) => { if (event.persisted) restore(); });
  });
})();
