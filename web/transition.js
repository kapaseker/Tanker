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
    let exitNote;
    // 首页退出确认弹框：存在时所有输入只作用于弹框，页面阶段保持不变。
    let dialog = null;
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

    // ---------- 首页退出确认弹框 ----------
    // 两段式操作：←→/Tab/Y/N 只切换选中按钮，Enter 触发选中按钮，鼠标点击直接触发；
    // Esc 或点击面板外取消并归还焦点。选中态由焦点表达，确认后沿用 exit() 既有流程。
    function openExitDialog() {
      if (dialog) return;
      dialog = document.createElement('div');
      dialog.className = 'exit-dialog';
      // 结构固定，一次生成；kbd 提示与实际绑定一致：Y 选中确认，N 选中取消
      dialog.innerHTML = `
        <div class="exit-dialog__panel" role="dialog" aria-modal="true" aria-labelledby="exit-dialog-title">
          <h2 class="exit-dialog__title" id="exit-dialog-title">确认退出游戏吗？</h2>
          <div class="exit-dialog__actions">
            <button type="button" class="exit-dialog__btn" data-role="cancel">取消 <kbd>N</kbd></button>
            <button type="button" class="exit-dialog__btn" data-role="confirm">确认 <kbd>Y</kbd></button>
          </div>
        </div>`;
      dialog.querySelector('[data-role="cancel"]').addEventListener('click', closeDialog);
      dialog.querySelector('[data-role="confirm"]').addEventListener('click', () => {
        // 确认：移除弹框后进入黑场退出流程，焦点不再归还电源框
        dialog.remove();
        dialog = null;
        exit();
      });
      document.body.append(dialog);
      // 初始选中确认按钮：打开即 Enter 可退出
      dialog.querySelector('[data-role="confirm"]').focus({ preventScroll: true });
    }

    // 取消路径：移除弹框并把焦点交还触发它的电源框
    function closeDialog() {
      dialog?.remove();
      dialog = null;
      back.focus({ preventScroll: true });
    }

    // 首页退出：沿用切页淡出节奏到黑场后尝试关闭窗口；
    // 设计稿在浏览器里通常无法关闭窗口，此时留在黑场给出提示，点击或 ESC 可返回主菜单。
    function exit() {
      root.dataset.transition = 'exit';
      fade(1, () => {
        window.close();
        exitNote = document.createElement('p');
        exitNote.className = 'exit-note';
        exitNote.textContent = '已退出 · 按 ESC 返回主菜单';
        exitNote.setAttribute('role', 'status');
        document.body.append(exitNote);
      });
    }

    function resume() {
      exitNote?.remove();
      exitNote = null;
      fade(0, restore);
    }

    function navigate(link) {
      if (['out', 'waiting', 'exit'].includes(root.dataset.transition)) return;
      const url = new URL(link.href);
      const name = url.pathname.split('/').pop();
      const titles = { 'index.html': '主菜单', 'model-gallery.html': '模型画廊', 'model-detail.html': '模型详情' };
      if (!titles[name] || url.origin !== location.origin) return;
      savedFocus = document.activeElement;
      destination = url.href;
      // 详情页淡出后显示的是具体型号而非通用名，预览会闪现"模型详情"，故不替换标题
      if (name !== 'model-detail.html') title.textContent = titles[name];
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
      // 弹框打开时：面板内点击交给按钮自身的监听（确认/取消），面板外一律取消
      if (dialog) {
        if (event.target.closest('.exit-dialog__panel')) return;
        event.preventDefault();
        event.stopImmediatePropagation();
        closeDialog();
        return;
      }
      if (root.dataset.transition === 'waiting') {
        event.preventDefault();
        event.stopImmediatePropagation();
        return;
      }
      if (root.dataset.transition === 'exit') {
        event.preventDefault();
        event.stopImmediatePropagation();
        if (exitNote) resume();
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
      if (home && isBack) openExitDialog();
      else navigate(link);
    }, true);

    document.addEventListener('keydown', (event) => {
      const phase = root.dataset.transition;
      // 弹框打开时独占键盘：←→/Tab/Y/N 仅切换选中，Enter 触发选中按钮，Esc 取消，
      // 其余按键一律吞掉，防止穿透到菜单导航或切页逻辑。
      if (dialog) {
        event.preventDefault();
        event.stopImmediatePropagation();
        if (event.repeat) return;
        const btn = document.activeElement;
        const switchFocus = (role) => dialog.querySelector(`[data-role="${role}"]`)?.focus({ preventScroll: true });
        if (event.key === 'Escape') closeDialog();
        else if (event.key === 'ArrowLeft' || event.key === 'ArrowRight' || event.key === 'Tab') {
          switchFocus(btn?.dataset?.role === 'cancel' ? 'confirm' : 'cancel');
        } else if (event.key === 'y' || event.key === 'Y') switchFocus('confirm');
        else if (event.key === 'n' || event.key === 'N') switchFocus('cancel');
        else if (event.key === 'Enter') {
          if (btn?.dataset?.role === 'cancel') closeDialog();
          else if (btn?.dataset?.role === 'confirm') {
            // Enter 已被 preventDefault，原生 click 不会触发；显式走确认路径
            dialog.remove();
            dialog = null;
            exit();
          }
        }
        return;
      }
      if (phase === 'waiting') {
        event.preventDefault();
        event.stopImmediatePropagation();
        return;
      }
      if (phase === 'exit') {
        event.preventDefault();
        event.stopImmediatePropagation();
        if (event.key === 'Escape' && !event.repeat && exitNote) resume();
        return;
      }
      if (event.key === 'Escape') {
        event.preventDefault();
        event.stopImmediatePropagation();
        if (event.repeat) return;
        if (phase === 'out') cancel();
        else if (home) openExitDialog();
        else navigate(back);
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
        // 弹框打开时：面板内的指针事件放行给按钮，其余（含滚轮）一律吞掉
        if (dialog) {
          if (event.target.closest('.exit-dialog__panel') && type !== 'wheel') return;
          event.preventDefault();
          event.stopImmediatePropagation();
          return;
        }
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
