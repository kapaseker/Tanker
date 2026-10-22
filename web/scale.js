// 游戏式整体缩放：界面按 1280×720 参考分辨率做固定尺寸设计，
// 窗口窄于参考宽度时对整个页面设置 zoom 等比缩小（等价游戏 UiScale），
// 不做网页式的断点重排；不小于参考宽度时保持 1:1 设计尺寸居中。
(() => {
  const REFERENCE_WIDTH = 1280;
  const root = document.documentElement;

  const applyScale = () => {
    // 实测（Chromium）：documentElement.clientWidth 不随根元素 zoom 变化，
    // 可直接作为真实可用宽度，无需修正因子，也不会形成缩放正反馈
    root.style.zoom = Math.min(1, root.clientWidth / REFERENCE_WIDTH);
  };

  window.addEventListener('resize', applyScale);
  applyScale();
})();
