// Type-safe bridge for Tauri v2 IPC commands and events

declare global {
  interface Window {
    __TAURI__?: {
      core: {
        invoke: <T = any>(cmd: string, args?: Record<string, any>) => Promise<T>;
      };
      event?: {
        listen: (event: string, handler: (event: { payload: any }) => void) => Promise<() => void>;
      };
    };
    __rift_emit?: (event: string, payload?: any) => void;
  }
}

export async function invokeTauri<T = any>(cmd: string, args?: Record<string, any>): Promise<T> {
  if (typeof window !== 'undefined' && window.__TAURI__?.core) {
    try {
      return await window.__TAURI__.core.invoke<T>(cmd, args);
    } catch (err) {
      console.error(`[Tauri IPC] ${cmd} failed:`, err);
      throw err;
    }
  }

  // Safe browser dev mode fallbacks
  console.warn(`[Tauri Mock] ${cmd} called with`, args);
  if (cmd === 'getDashboardStats') {
    return {
      totalWords: 3420,
      totalSpeakingSeconds: 1540.5,
      totalRecordings: 28,
      avgPaceWpm: 133,
      todayRequests: 5,
      dailyLimit: 2000,
      currentModel: 'groq/whisper-large-v3-turbo',
      transcriptionMode: 'cloud',
      remainingUsage: {
        formattedTime: 'Unlimited',
        subtext: 'Groq Cloud Tier',
        dotColor: '#45dfa9',
        provider: 'groq',
      },
    } as any;
  }
  if (cmd === 'getSettings' || cmd === 'loadSettings') {
    return {
      push_to_talk_key: 'right alt',
      push_to_talk_mode: 'single',
      push_to_talk_combo: 'ctrl+win',
      pushToTalkKey: 'right alt',
      pushToTalkMode: 'single',
      pushToTalkCombo: 'ctrl+win',
      transcription_mode: 'cloud',
      transcriptionMode: 'cloud',
      stt_provider: 'groq',
      sttProvider: 'groq',
      inference_model: 'whisper-large-v3-turbo',
      inferenceModel: 'whisper-large-v3-turbo',
      cloudModel: 'whisper-large-v3-turbo',
      onboarding_completed: true,
      onboardingCompleted: true,
      recording_sounds: true,
      recordingSounds: true,
      soundFeedback: true,
      sound_volume: 0.6,
      soundVolume: 0.6,
      widget_visibility: 'always',
      widgetVisibility: 'always',
      widget_style: 'active_waveform',
      widgetStyle: 'active_waveform',
      mic_boost: '0dB',
      micBoost: '0dB',
      launch_on_startup: false,
      launchOnStartup: false,
      launch_at_login: false,
      launchAtLogin: false,
      pause_other_audio_while_talking: true,
      pauseOtherAudioWhileTalking: true,
    } as any;
  }
  if (cmd === 'getSnippets') {
    return [] as any;
  }
  if (cmd === 'getHistory') {
    return [] as any;
  }
  if (cmd === 'getDictionary') {
    return [] as any;
  }
  return null as any;
}

// Global Event Listener registration with native Tauri support
export function listenTauriEvent<T = any>(event: string, handler: (payload: T) => void): () => void {
  let isCleanedUp = false;
  let tauriUnlisten: (() => void) | null = null;

  // 1. Native Tauri v2 Event Listener
  if (typeof window !== 'undefined' && window.__TAURI__?.event?.listen) {
    window.__TAURI__.event
      .listen(event, (ev) => {
        if (!isCleanedUp) {
          handler(ev.payload);
        }
      })
      .then((unlistenFn) => {
        if (isCleanedUp) {
          unlistenFn();
        } else {
          tauriUnlisten = unlistenFn;
        }
      })
      .catch((err) => console.warn(`[Tauri Event] Listen failed for ${event}:`, err));
  }

  // 2. Custom DOM Event Fallback
  const customHandler = (e: CustomEvent) => {
    if (!isCleanedUp) {
      handler(e.detail);
    }
  };
  window.addEventListener(`rift:${event}`, customHandler as EventListener);

  return () => {
    isCleanedUp = true;
    window.removeEventListener(`rift:${event}`, customHandler as EventListener);
    if (tauriUnlisten) {
      tauriUnlisten();
    }
  };
}

// Register global emitter bridge if Rust evaluates window.__rift_emit
if (typeof window !== 'undefined') {
  window.__rift_emit = (event: string, payload?: any) => {
    window.dispatchEvent(new CustomEvent(`rift:${event}`, { detail: payload }));
  };
}
