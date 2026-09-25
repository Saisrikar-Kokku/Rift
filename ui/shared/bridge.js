/**
 * Rift Desktop Bridge Client
 * Supports Tauri 2.0, pywebview (WebView2), and QWebChannel to provide a clean,
 * Promise-based and event-driven API for all 9 screens and the widget.
 * 
 * Usage:
 *   await window.rift.ready();
 *   const config = await window.rift.getWidgetConfig();
 *   window.rift.on("recordingStarted", () => { ... });
 */

(function () {
  'use strict';

  let rawBridge = null;
  let isChannelReady = false;
  let backendType = null; // 'tauri' | 'pywebview' | 'qwebchannel' | null
  const readyCallbacks = [];
  const pendingSignalListeners = new Map(); // eventName -> Set of callbacks

  // Global event dispatcher invoked by backend
  window.__rift_emit = function (eventName, payload) {
    const callbacks = pendingSignalListeners.get(eventName);
    if (callbacks) {
      for (const cb of callbacks) {
        try {
          cb(payload);
        } catch (err) {
          console.error(`[Rift Bridge] Error in listener for '${eventName}':`, err);
        }
      }
    }
  };

  function notifyReady(type, bridgeObj) {
    if (isChannelReady) return;
    isChannelReady = true;
    backendType = type;
    rawBridge = bridgeObj;

    console.info(`[Rift Bridge] Connected to backend via ${type}.`);

    while (readyCallbacks.length > 0) {
      const cb = readyCallbacks.shift();
      try {
        cb(rawBridge);
      } catch (err) {
        console.error('[Rift Bridge] Error in ready callback:', err);
      }
    }
  }

  const activeTauriListeners = new Set();

  function listenTauriEvent(ev) {
    if (activeTauriListeners.has(ev)) return;
    if (window.__TAURI__ && window.__TAURI__.event && typeof window.__TAURI__.event.listen === 'function') {
      activeTauriListeners.add(ev);
      window.__TAURI__.event.listen(ev, (e) => {
        window.__rift_emit(ev, e.payload);
      });
    }
  }

  function initTauri() {
    if (window.__TAURI__ && window.__TAURI__.core) {
      notifyReady('tauri', window.__TAURI__.core);

      const events = [
        'recordingStarted', 'recordingStopped', 'recordingCancelled', 'recordingSaved',
        'audioLevel', 'transcriptionSuccess', 'transcriptionError', 'transcriptionComplete',
        'repositionWidget', 'targetCaptured', 'settingsChanged', 'widgetConfigChanged',
        'hotkeyCaptured', 'hotkeyVerified', 'clipboardChanged', 'toast'
      ];
      for (const ev of events) {
        listenTauriEvent(ev);
      }
      return true;
    }
    return false;
  }

  function initPyWebView() {
    if (window.pywebview && window.pywebview.api) {
      notifyReady('pywebview', window.pywebview.api);
      return true;
    }
    return false;
  }

  function initWebChannel() {
    if (typeof QWebChannel === 'undefined') return false;
    const transport = window.qt && window.qt.webChannelTransport;
    if (!transport) return false;

    new QWebChannel(transport, function (channel) {
      if (channel && channel.objects && channel.objects.bridge) {
        notifyReady('qwebchannel', channel.objects.bridge);
      }
    });
    return true;
  }

  // Define rift base object
  const riftTarget = {
    get isReady() {
      return isChannelReady;
    },

    get rawBridge() {
      return rawBridge;
    },

    get backendType() {
      return backendType;
    },

    ready: function (callback) {
      return new Promise((resolve) => {
        const handler = () => {
          if (typeof callback === 'function') {
            callback(rawBridge);
          }
          resolve(rawBridge);
        };

        if (isChannelReady) {
          handler();
        } else {
          readyCallbacks.push(handler);
        }
      });
    },

    on: function (eventName, callback) {
      if (typeof callback !== 'function') {
        throw new TypeError('[Rift Bridge] Signal listener must be a function.');
      }

      if (!pendingSignalListeners.has(eventName)) {
        pendingSignalListeners.set(eventName, new Set());
      }
      pendingSignalListeners.get(eventName).add(callback);

      listenTauriEvent(eventName);

      return () => this.off(eventName, callback);
    },

    off: function (eventName, callback) {
      if (pendingSignalListeners.has(eventName)) {
        pendingSignalListeners.get(eventName).delete(callback);
      }
    }
  };

  const commandParamMap = {
    getHotkeys: [],
    saveHotkey: ['action', 'combo'],
    clearHotkey: ['action'],
    verifyHotkey: ['action'],
    startHotkeyCapture: ['mode'],
    cancelHotkeyCapture: [],
    setMicrophone: ['deviceId'],
    getGroqKeyStatus: [],
    testGroqKey: ['key'],
    saveGroqKey: ['key'],
    deleteGroqKey: [],
    getOpenRouterKeyStatus: [],
    testOpenRouterKey: ['key'],
    saveOpenRouterKey: ['key'],
    deleteOpenRouterKey: [],
    getOpenRouterApiKey: [],
    saveOpenRouterApiKey: ['key'],
    getSettings: [],
    openOnboardingWindow: [],
    setWidgetVisibility: ['mode'],
    setFollowCursor: ['enabled'],
    setLockPosition: ['enabled'],
    persistWidgetPosition: ['x', 'y'],
    deleteHistoryEntry: ['id'],
    updateDictionaryEntry: ['id', 'patch'],
    deleteDictionaryEntry: ['id'],
    addDictionaryEntry: ['entry'],
    downloadLocalModel: ['profile'],
    deleteLocalModel: ['profile'],
    verifyLocalModel: ['profile'],
    setLocalInferenceProfile: ['profile'],
    setTranscriptionMode: ['mode'],
    testEnhance: ['text', 'instructions'],
    testCleanup: ['text'],
    testDictionary: ['text'],
    copyToClipboard: ['text'],
    triggerWidgetTestState: ['state'],
    saveSettings: ['patch'],
    updateSettings: ['patch'],
    cancelRecording: [],
    startRecording: [],
    stopRecording: [],
    toggleRecording: [],
    getHistory: ['filter'],
    clearHistory: [],
    getDashboardStats: [],
    getModelRemainingUsage: [],
    setWidgetDropdownOpen: ['open'],
    openScratchpadWindow: [],
    closeScratchpadWindow: [],
    minimizeScratchpadWindow: [],
    toggleScratchpadPin: ['pin'],
    startDragging: [],
    startWidgetDrag: [],
    startScratchpadDragging: [],
    moveWidgetBy: ['dx', 'dy'],
    saveCurrentWidgetPosition: [],
    resetWidgetPosition: [],
    insertScratchpadText: ['text'],
    polishSelectedText: [],
    openSpotlightWindow: [],
    closeSpotlightWindow: [],
    askSpotlightAi: ['prompt', 'context'],
    insertSpotlightText: ['text'],
    testAudioCue: ['soundType', 'volume'],
    getSnippets: [],
    saveSnippet: ['snippet'],
    deleteSnippet: ['id'],
    getClipboardHistory: ['limit', 'category', 'query'],
    toggleClipPin: ['id'],
    deleteClip: ['id'],
    clearUnpinnedClips: [],
    copyClipById: ['id'],
    pasteClipById: ['id'],
    getClipboardCounts: [],
    retryLastTranscription: [],
    hasSavedRecording: [],
  };

  // Wrap in a Proxy to dynamically handle any method invocation
  window.rift = new Proxy(riftTarget, {
    get(target, prop, receiver) {
      if (prop in target) {
        return Reflect.get(target, prop, receiver);
      }

      // Automatically forward method call as a Promise
      return function (...args) {
        return target.ready().then((bridge) => {
          if (backendType === 'tauri') {
            const cmd = String(prop);
            let payload = {};
            const paramNames = commandParamMap[cmd];
            if (paramNames) {
              paramNames.forEach((name, idx) => {
                if (args[idx] !== undefined) {
                  payload[name] = args[idx];
                }
              });
            } else if (args.length === 1) {
              const firstArg = args[0];
              if (firstArg !== undefined) {
                if (typeof firstArg === 'object' && !Array.isArray(firstArg)) {
                  payload = firstArg;
                } else {
                  payload = { value: firstArg };
                }
              }
            }

            // Ensure id is always stringified for Tauri commands expecting String id
            if ((cmd === 'deleteHistoryEntry' || cmd === 'deleteDictionaryEntry' || cmd === 'updateDictionaryEntry') && payload.id !== undefined && payload.id !== null) {
              payload.id = String(payload.id);
            }

            return window.__TAURI__.core.invoke(cmd, payload);
          }

          if (backendType === 'pywebview') {
            const api = (window.pywebview && window.pywebview.api) || bridge;
            if (!api || typeof api[prop] !== 'function') {
              throw new Error(`[Rift Bridge] Method '${String(prop)}' not found on pywebview API.`);
            }
            return api[prop](...args);
          }

          return new Promise((resolve, reject) => {
            if (!bridge || typeof bridge[prop] !== 'function') {
              reject(new Error(`[Rift Bridge] Method '${String(prop)}' not found on bridge.`));
              return;
            }
            try {
              bridge[prop](...args, function (result) {
                resolve(result);
              });
            } catch (err) {
              reject(err);
            }
          });
        });
      };
    }
  });

  // Attempt connections
  if (!initTauri() && !initPyWebView()) {
    initWebChannel();
  }

  // Fast polling to immediately catch Tauri injection
  let tauriPollCount = 0;
  const tauriPoll = setInterval(() => {
    tauriPollCount++;
    if (initTauri() || isChannelReady || tauriPollCount > 50) {
      clearInterval(tauriPoll);
    }
  }, 20);

  window.addEventListener('pywebviewready', () => {
    if (!isChannelReady) initPyWebView();
  });

  window.addEventListener('DOMContentLoaded', () => {
    if (!isChannelReady) {
      if (!initTauri() && !initPyWebView()) initWebChannel();
    }
  });

  window.addEventListener('load', () => {
    if (!isChannelReady) {
      if (!initTauri() && !initPyWebView()) initWebChannel();
    }
  });
})();
