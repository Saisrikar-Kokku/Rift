/**
 * Rift Desktop - Premium Minimal Toast Notification Component
 * Provides ultra-sleek, classic obsidian glassmorphic notifications.
 */

(function () {
  'use strict';

  let container = null;

  function ensureContainer() {
    if (!container || !document.body.contains(container)) {
      container = document.createElement('div');
      container.id = 'rift-toast-container';
      container.className = 'rift-toast-container';
      container.style.cssText = [
        'position: fixed',
        'bottom: 24px',
        'right: 24px',
        'z-index: 99999',
        'display: flex',
        'flex-direction: column',
        'gap: 10px',
        'pointer-events: none',
        'max-width: 400px',
        'width: calc(100% - 48px)',
        'font-family: "Manrope", -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Inter", sans-serif'
      ].join(';');
      document.body.appendChild(container);
    }
    return container;
  }

  function showToast(options, optionalType) {
    if (typeof options === 'string') {
      options = { message: options, type: optionalType || 'success' };
    }
    const {
      message = '',
      type = 'success', // 'success' | 'error' | 'info' | 'warning' | 'retry'
      duration = (options.canRetry ? 9000 : 4000),
      canRetry = false,
      actionText = canRetry ? 'Retry ↻' : options.actionText,
      onAction = null,
    } = options;

    const parent = ensureContainer();
    const toast = document.createElement('div');
    toast.className = 'rift-toast-item';

    // Theme color configurations
    let iconSvg = '';
    let iconColor = '#34d399';
    let borderColor = 'rgba(52, 211, 153, 0.25)';
    let glowShadow = '0 14px 38px rgba(0, 0, 0, 0.65)';

    if (type === 'error') {
      iconColor = '#f87171';
      borderColor = 'rgba(239, 68, 68, 0.3)';
      glowShadow = '0 14px 38px rgba(0, 0, 0, 0.65), 0 0 20px rgba(239, 68, 68, 0.1)';
      iconSvg = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" style="width: 17px; height: 17px;"><circle cx="12" cy="12" r="10"></circle><line x1="15" y1="9" x2="9" y2="15"></line><line x1="9" y1="9" x2="15" y2="15"></line></svg>`;
    } else if (type === 'warning' || type === 'retry' || canRetry) {
      iconColor = '#fbbf24';
      borderColor = 'rgba(245, 158, 11, 0.35)';
      glowShadow = '0 14px 40px rgba(0, 0, 0, 0.7), 0 0 22px rgba(245, 158, 11, 0.12)';
      iconSvg = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.3" stroke-linecap="round" stroke-linejoin="round" style="width: 17px; height: 17px;"><path d="M21.5 2v6h-6M21.34 15.57a10 10 0 1 1-.57-8.38l5.67-5.67"/></svg>`;
    } else if (type === 'info') {
      iconColor = '#818cf8';
      borderColor = 'rgba(129, 140, 248, 0.25)';
      iconSvg = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" style="width: 17px; height: 17px;"><circle cx="12" cy="12" r="10"></circle><line x1="12" y1="16" x2="12" y2="12"></line><line x1="12" y1="8" x2="12.01" y2="8"></line></svg>`;
    } else {
      // Success
      iconSvg = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round" style="width: 17px; height: 17px;"><polyline points="20 6 9 17 4 12"></polyline></svg>`;
    }

    toast.style.cssText = [
      'pointer-events: auto',
      'display: flex',
      'align-items: center',
      'gap: 12px',
      'padding: 10px 14px 10px 14px',
      'border-radius: 12px',
      'background: rgba(18, 18, 26, 0.94)',
      `border: 1px solid ${borderColor}`,
      `box-shadow: ${glowShadow}`,
      'backdrop-filter: blur(20px)',
      '-webkit-backdrop-filter: blur(20px)',
      'transition: all 260ms cubic-bezier(0.16, 1, 0.3, 1)',
      'box-sizing: border-box',
      'width: 100%',
      'opacity: 0',
      'transform: translateY(10px) scale(0.97)',
    ].join(';');

    let actionButtonHtml = '';
    if (canRetry || actionText) {
      actionButtonHtml = `
        <button class="rift-toast-action-btn" type="button" style="
          display: inline-flex;
          align-items: center;
          gap: 5px;
          background: rgba(245, 158, 11, 0.16);
          border: 1px solid rgba(245, 158, 11, 0.45);
          color: #fbbf24;
          border-radius: 999px;
          padding: 3px 9px;
          font-size: 11px;
          font-weight: 600;
          cursor: pointer;
          transition: all 140ms ease;
          outline: none;
          white-space: nowrap;
          flex-shrink: 0;
        ">${actionText}</button>
      `;
    }

    toast.innerHTML = `
      <div style="color: ${iconColor}; display: flex; align-items: center; justify-content: center; flex-shrink: 0;">
        ${iconSvg}
      </div>
      <div style="font-size: 12.5px; font-weight: 500; color: #f4f4f5; flex: 1; line-height: 1.35; letter-spacing: -0.01em; word-break: break-word;">
        ${message}
      </div>
      ${actionButtonHtml}
      <button class="rift-toast-close-btn" type="button" title="Dismiss" style="
        background: transparent;
        border: none;
        color: #71717a;
        cursor: pointer;
        padding: 2px 4px;
        display: flex;
        align-items: center;
        justify-content: center;
        border-radius: 4px;
        transition: color 120ms ease;
        flex-shrink: 0;
      ">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" style="width: 14px; height: 14px;">
          <line x1="18" y1="6" x2="6" y2="18"></line>
          <line x1="6" y1="6" x2="18" y2="18"></line>
        </svg>
      </button>
    `;

    const closeBtn = toast.querySelector('.rift-toast-close-btn');
    const actionBtn = toast.querySelector('.rift-toast-action-btn');
    let timer = null;

    function dismiss() {
      if (timer) clearTimeout(timer);
      toast.style.opacity = '0';
      toast.style.transform = 'translateY(6px) scale(0.96)';
      setTimeout(() => {
        if (toast.parentNode) {
          toast.parentNode.removeChild(toast);
        }
      }, 260);
    }

    if (closeBtn) {
      closeBtn.addEventListener('mouseenter', () => { closeBtn.style.color = '#e4e4e7'; });
      closeBtn.addEventListener('mouseleave', () => { closeBtn.style.color = '#71717a'; });
      closeBtn.addEventListener('click', dismiss);
    }

    if (actionBtn) {
      actionBtn.addEventListener('mouseenter', () => {
        actionBtn.style.background = 'rgba(245, 158, 11, 0.28)';
        actionBtn.style.borderColor = 'rgba(245, 158, 11, 0.7)';
        actionBtn.style.transform = 'translateY(-1px) scale(1.02)';
        actionBtn.style.color = '#fffbeb';
      });
      actionBtn.addEventListener('mouseleave', () => {
        actionBtn.style.background = 'rgba(245, 158, 11, 0.16)';
        actionBtn.style.borderColor = 'rgba(245, 158, 11, 0.45)';
        actionBtn.style.transform = '';
        actionBtn.style.color = '#fbbf24';
      });
      actionBtn.addEventListener('click', (e) => {
        e.stopPropagation();
        if (typeof onAction === 'function') {
          onAction();
        } else if (canRetry || actionText.includes('Retry')) {
          if (window.rift && typeof window.rift.retryLastTranscription === 'function') {
            window.rift.retryLastTranscription();
          }
        }
        dismiss();
      });
    }

    parent.appendChild(toast);

    requestAnimationFrame(() => {
      toast.style.opacity = '1';
      toast.style.transform = 'translateY(0) scale(1)';
    });

    if (duration > 0) {
      timer = setTimeout(dismiss, duration);
    }

    return { dismiss };
  }

  // Auto-subscribe to backend toast event if rift bridge is available
  function initBridgeListener() {
    if (window.rift && typeof window.rift.on === 'function') {
      window.rift.on('toast', (payload) => {
        try {
          const data = typeof payload === 'string' ? JSON.parse(payload) : payload;
          if (data && data.message) {
            showToast(data);
          }
        } catch (e) {
          console.warn('[Toast] Error parsing toast event:', e);
        }
      });
    }
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', initBridgeListener);
  } else {
    initBridgeListener();
  }

  window.showToast = showToast;
})();
