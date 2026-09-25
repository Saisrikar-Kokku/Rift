/**
 * Rift Desktop - Modal Components (Phase 12)
 * Provides robust, properly constrained confirmation modals and dictionary phrase edit modals.
 */

(function () {
  'use strict';

  // Ensure modal.css is loaded
  if (!document.getElementById('rift-modal-styles')) {
    const link = document.createElement('link');
    link.id = 'rift-modal-styles';
    link.rel = 'stylesheet';
    link.href = '../shared/modal.css';
    document.head.appendChild(link);
  }

  function showConfirmModal(options) {
    return new Promise((resolve) => {
      let opts = options;
      if (typeof options === 'string') {
        opts = { message: options };
      } else if (!options) {
        opts = {};
      }
      const {
        title = 'Are you sure?',
        message = 'This action cannot be undone.',
        confirmText = 'Confirm',
        cancelText = 'Cancel',
        isDestructive = true,
      } = opts;

      const overlay = document.createElement('div');
      overlay.className = 'rift-modal-overlay';
      overlay.style.cssText = 'position:fixed;inset:0;top:0;left:0;right:0;bottom:0;width:100vw;height:100vh;z-index:9999;background:rgba(10,10,16,0.8);backdrop-filter:blur(10px);display:flex;align-items:center;justify-content:center;padding:16px;box-sizing:border-box;opacity:0;transition:opacity 0.2s ease;';

      const confirmBtnBg = isDestructive
        ? 'background:#ef4444;color:#fff;'
        : 'background:#cabeff;color:#1d0061;';

      overlay.innerHTML = `
        <div class="rift-modal-card confirm-card" style="position:relative;width:100%;max-width:440px;max-height:85vh;overflow-y:auto;box-sizing:border-box;background:#1c1b22;border:1px solid rgba(255,255,255,0.12);border-radius:16px;padding:24px;box-shadow:0 20px 50px rgba(0,0,0,0.6);display:flex;flex-direction:column;gap:16px;transform:scale(0.95);transition:transform 0.2s ease;">
          <div style="display:flex;align-items:flex-start;gap:12px;">
            <div style="padding:8px;border-radius:12px;display:flex;align-items:center;justify-content:center;${isDestructive ? 'background:rgba(239,68,68,0.15);color:#ef4444;' : 'background:rgba(202,190,255,0.15);color:#cabeff;'}">
              <span class="material-symbols-outlined" style="font-size:22px;">${isDestructive ? 'warning' : 'help'}</span>
            </div>
            <div style="flex:1;min-width:0;">
              <h3 style="margin:0;font-size:16px;font-weight:600;color:#e4e1ea;">${title}</h3>
              <p style="margin:6px 0 0 0;font-size:13px;line-height:1.5;color:#cac4d7;">${message}</p>
            </div>
          </div>
          <div style="display:flex;align-items:center;justify-content:end;gap:10px;margin-top:8px;padding-top:12px;border-top:1px solid rgba(255,255,255,0.08);">
            <button id="modalCancelBtn" type="button" style="padding:8px 16px;border-radius:8px;font-size:13px;font-weight:500;color:#cac4d7;background:transparent;border:1px solid rgba(255,255,255,0.1);cursor:pointer;transition:all 0.15s;">
              ${cancelText}
            </button>
            <button id="modalConfirmBtn" type="button" style="padding:8px 18px;border-radius:8px;font-size:13px;font-weight:600;border:none;cursor:pointer;transition:all 0.15s;${confirmBtnBg}">
              ${confirmText}
            </button>
          </div>
        </div>
      `;

      document.body.appendChild(overlay);

      const card = overlay.querySelector('.rift-modal-card');
      const cancelBtn = overlay.querySelector('#modalCancelBtn');
      const confirmBtn = overlay.querySelector('#modalConfirmBtn');

      function cleanup(result) {
        window.removeEventListener('keydown', onKeyDown);
        overlay.style.opacity = '0';
        if (card) card.style.transform = 'scale(0.95)';
        setTimeout(() => {
          if (overlay.parentNode) overlay.parentNode.removeChild(overlay);
          resolve(result);
        }, 150);
      }

      function onKeyDown(e) {
        if (e.key === 'Escape') {
          cleanup(false);
        } else if (e.key === 'Enter') {
          cleanup(true);
        }
      }

      cancelBtn.addEventListener('click', () => cleanup(false));
      confirmBtn.addEventListener('click', () => cleanup(true));
      overlay.addEventListener('click', (e) => {
        if (e.target === overlay) cleanup(false);
      });

      window.addEventListener('keydown', onKeyDown);

      requestAnimationFrame(() => {
        overlay.style.opacity = '1';
        if (card) card.style.transform = 'scale(1)';
        confirmBtn.focus();
      });
    });
  }

  function showDictionaryModal(initialData = null) {
    return new Promise((resolve) => {
      const isEdit = initialData && initialData.id;
      const title = isEdit ? 'Edit Dictionary Rule' : 'Add Dictionary Rule';
      const phrase = (initialData && initialData.phrase) || '';
      const replacement = (initialData && initialData.replacement) || '';
      const tag = (initialData && initialData.tag) || '';

      const overlay = document.createElement('div');
      overlay.className = 'rift-modal-overlay';
      overlay.style.cssText = 'position:fixed;inset:0;top:0;left:0;right:0;bottom:0;width:100vw;height:100vh;z-index:9999;background:rgba(10,10,16,0.8);backdrop-filter:blur(10px);display:flex;align-items:center;justify-content:center;padding:16px;box-sizing:border-box;opacity:0;transition:opacity 0.2s ease;';

      overlay.innerHTML = `
        <div class="rift-modal-card" style="position:relative;width:100%;max-width:520px;max-height:85vh;overflow-y:auto;box-sizing:border-box;background:#1c1b22;border:1px solid rgba(255,255,255,0.12);border-radius:16px;padding:24px;box-shadow:0 20px 50px rgba(0,0,0,0.6);display:flex;flex-direction:column;gap:18px;transform:scale(0.95);transition:transform 0.2s ease;">
          <div style="display:flex;align-items:center;justify-content:space-between;padding-bottom:12px;border-bottom:1px solid rgba(255,255,255,0.08);">
            <div style="display:flex;align-items:center;gap:8px;">
              <span class="material-symbols-outlined" style="font-size:22px;color:#cabeff;">book_2</span>
              <h3 style="margin:0;font-size:16px;font-weight:600;color:#e4e1ea;">${title}</h3>
            </div>
            <button id="dictModalCloseBtn" type="button" style="background:transparent;border:none;color:#cac4d7;padding:4px;border-radius:6px;cursor:pointer;display:flex;align-items:center;justify-content:center;">
              <span class="material-symbols-outlined" style="font-size:18px;">close</span>
            </button>
          </div>
          
          <form id="dictModalForm" style="display:flex;flex-direction:column;gap:16px;margin:0;">
            <div style="display:flex;flex-direction:column;gap:6px;">
              <label style="font-size:11px;font-weight:600;color:#cac4d7;text-transform:uppercase;letter-spacing:0.05em;">Spoken Phrase / Trigger</label>
              <input id="dictPhraseInput" type="text" required placeholder="e.g. rift api or git commit" value="${phrase.replace(/"/g, '&quot;')}"
                style="width:100%;background:#131319;border:1px solid rgba(255,255,255,0.12);border-radius:8px;padding:10px 12px;font-size:13px;color:#e4e1ea;box-sizing:border-box;font-family:monospace;outline:none;" />
              <span style="font-size:11px;color:#938ea1;">What you say into the microphone (case-insensitive)</span>
            </div>

            <div style="display:flex;flex-direction:column;gap:6px;">
              <label style="font-size:11px;font-weight:600;color:#cac4d7;text-transform:uppercase;letter-spacing:0.05em;">Replacement Output</label>
              <input id="dictReplacementInput" type="text" required placeholder="e.g. @Rift API or git commit -m" value="${replacement.replace(/"/g, '&quot;')}"
                style="width:100%;background:#131319;border:1px solid rgba(255,255,255,0.12);border-radius:8px;padding:10px 12px;font-size:13px;color:#e4e1ea;box-sizing:border-box;font-family:monospace;outline:none;" />
              <span style="font-size:11px;color:#938ea1;">The exact text to inject into your target application</span>
            </div>

            <div style="display:flex;flex-direction:column;gap:6px;">
              <label style="font-size:11px;font-weight:600;color:#cac4d7;text-transform:uppercase;letter-spacing:0.05em;">Category Tag (Optional)</label>
              <input id="dictTagInput" type="text" placeholder="e.g. CLI & Git, Dev Mentions, Syntax" value="${tag.replace(/"/g, '&quot;')}"
                style="width:100%;background:#131319;border:1px solid rgba(255,255,255,0.12);border-radius:8px;padding:10px 12px;font-size:13px;color:#e4e1ea;box-sizing:border-box;outline:none;" />
            </div>

            <div style="display:flex;align-items:center;justify-content:flex-end;gap:10px;padding-top:12px;border-top:1px solid rgba(255,255,255,0.08);margin-top:4px;">
              <button type="button" id="dictCancelBtn" style="padding:8px 16px;border-radius:8px;font-size:13px;font-weight:500;color:#cac4d7;background:transparent;border:1px solid rgba(255,255,255,0.1);cursor:pointer;">
                Cancel
              </button>
              <button type="submit" id="dictSubmitBtn" style="padding:8px 20px;border-radius:8px;font-size:13px;font-weight:600;background:#cabeff;color:#1d0061;border:none;cursor:pointer;display:flex;align-items:center;gap:6px;box-shadow:0 4px 12px rgba(202,190,255,0.25);">
                <span class="material-symbols-outlined" style="font-size:16px;">save</span>
                <span>${isEdit ? 'Update Rule' : 'Add Rule'}</span>
              </button>
            </div>
          </form>
        </div>
      `;

      document.body.appendChild(overlay);

      const card = overlay.querySelector('.rift-modal-card');
      const form = overlay.querySelector('#dictModalForm');
      const closeBtn = overlay.querySelector('#dictModalCloseBtn');
      const cancelBtn = overlay.querySelector('#dictCancelBtn');
      const phraseInput = overlay.querySelector('#dictPhraseInput');
      const replacementInput = overlay.querySelector('#dictReplacementInput');
      const tagInput = overlay.querySelector('#dictTagInput');

      function cleanup(result) {
        window.removeEventListener('keydown', onKeyDown);
        overlay.style.opacity = '0';
        if (card) card.style.transform = 'scale(0.95)';
        setTimeout(() => {
          if (overlay.parentNode) overlay.parentNode.removeChild(overlay);
          resolve(result);
        }, 150);
      }

      function onKeyDown(e) {
        if (e.key === 'Escape') {
          cleanup(null);
        }
      }

      form.addEventListener('submit', (e) => {
        e.preventDefault();
        const p = phraseInput.value.trim();
        const r = replacementInput.value;
        const t = tagInput.value.trim();

        if (!p) {
          phraseInput.focus();
          return;
        }

        cleanup({
          phrase: p,
          replacement: r,
          tag: t || null,
        });
      });

      closeBtn.addEventListener('click', () => cleanup(null));
      cancelBtn.addEventListener('click', () => cleanup(null));
      overlay.addEventListener('click', (e) => {
        if (e.target === overlay) cleanup(null);
      });

      window.addEventListener('keydown', onKeyDown);

      requestAnimationFrame(() => {
        overlay.style.opacity = '1';
        if (card) card.style.transform = 'scale(1)';
        phraseInput.focus();
      });
    });
  }

  window.showConfirmModal = showConfirmModal;
  window.showDictionaryModal = showDictionaryModal;
})();
