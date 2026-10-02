import React, { createContext, useContext, useEffect, useState } from 'react';
import { invokeTauri, listenTauriEvent } from '../lib/tauriBridge';

export interface AppSettings {
  push_to_talk_key: string;
  push_to_talk_mode: string;
  push_to_talk_combo: string;
  toggle_recording_key?: string | null;
  microphone_device_id?: string | null;
  widget_visibility: string;
  widget_style: string;
  follow_cursor_across_monitors: boolean;
  lock_widget_position: boolean;
  copy_instead_of_type: boolean;
  launch_on_startup: boolean;
  launch_at_login: boolean;
  transcription_mode: string;
  stt_provider: string;
  inference_model: string;
  local_inference_profile?: string;
  recording_sounds: boolean;
  sound_volume: number;
  pause_other_audio_while_talking: boolean;
  auto_enhance_prompt: boolean;
  context_aware_formatting: boolean;
  formatting_instructions: string;
  theme: string;
  onboarding_completed: boolean;
  mic_boost: string;
  voice_commands_enabled: boolean;
  voice_snippets_enabled: boolean;
  quick_actions_enabled: boolean;
  clipboard_history_enabled: boolean;
  clipboard_max_items: number;
  tenglish_mode_enabled: boolean;
  tenglish_stt_tier: string;
  tenglish_translit_engine: string;
  noise_cancellation_enabled: boolean;
  ambient_memory_enabled: boolean;
  groq_api_key?: string;
  openrouter_api_key?: string;
  [key: string]: any;
}

const defaultSettings: AppSettings = {
  push_to_talk_key: 'right alt',
  push_to_talk_mode: 'single',
  push_to_talk_combo: 'ctrl+win',
  toggle_recording_key: null,
  microphone_device_id: null,
  widget_visibility: 'always',
  widget_style: 'active_waveform',
  follow_cursor_across_monitors: false,
  lock_widget_position: false,
  copy_instead_of_type: false,
  launch_on_startup: false,
  launch_at_login: false,
  transcription_mode: 'cloud',
  stt_provider: 'groq',
  inference_model: 'whisper-large-v3-turbo',
  local_inference_profile: 'balanced',
  recording_sounds: true,
  sound_volume: 0.6,
  pause_other_audio_while_talking: true,
  auto_enhance_prompt: false,
  context_aware_formatting: false,
  formatting_instructions: '',
  theme: 'obsidian',
  onboarding_completed: false,
  mic_boost: '0dB',
  voice_commands_enabled: false,
  voice_snippets_enabled: false,
  quick_actions_enabled: true,
  clipboard_history_enabled: true,
  clipboard_max_items: 50,
  tenglish_mode_enabled: false,
  tenglish_stt_tier: 'fast',
  tenglish_translit_engine: 'gemini-flash',
  noise_cancellation_enabled: true,
  ambient_memory_enabled: false,
};

function normalizeIncomingSettings(raw: any, prev: AppSettings): AppSettings {
  if (!raw || typeof raw !== 'object') return prev;

  const push_to_talk_key = raw.pushToTalkKey ?? raw.push_to_talk_key ?? prev.push_to_talk_key;
  const push_to_talk_mode = raw.pushToTalkMode ?? raw.push_to_talk_mode ?? prev.push_to_talk_mode;
  const push_to_talk_combo = raw.pushToTalkCombo ?? raw.push_to_talk_combo ?? prev.push_to_talk_combo;
  const toggle_recording_key = raw.toggleRecordingKey !== undefined ? raw.toggleRecordingKey : (raw.toggle_recording_key !== undefined ? raw.toggle_recording_key : prev.toggle_recording_key);
  const microphone_device_id = raw.microphoneDeviceId !== undefined ? raw.microphoneDeviceId : (raw.microphone_device_id !== undefined ? raw.microphone_device_id : prev.microphone_device_id);
  const widget_visibility = raw.widgetVisibility ?? raw.widget_visibility ?? prev.widget_visibility;
  const widget_style = raw.widgetStyle ?? raw.widget_style ?? prev.widget_style;
  const follow_cursor_across_monitors = raw.followCursorAcrossMonitors ?? raw.follow_cursor_across_monitors ?? prev.follow_cursor_across_monitors;
  const lock_widget_position = raw.lockWidgetPosition ?? raw.lock_widget_position ?? prev.lock_widget_position;
  const copy_instead_of_type = raw.copyInsteadOfType ?? raw.copy_instead_of_type ?? (raw.pasteDirectly !== undefined ? !raw.pasteDirectly : prev.copy_instead_of_type);
  const launch_on_startup = raw.launchOnStartup ?? raw.launch_on_startup ?? raw.launchAtLogin ?? prev.launch_on_startup;
  const launch_at_login = raw.launchAtLogin ?? raw.launch_at_login ?? raw.launchOnStartup ?? prev.launch_at_login;
  const transcription_mode = raw.transcriptionMode ?? raw.transcription_mode ?? prev.transcription_mode;
  const inference_model = raw.inferenceModel ?? raw.cloudModel ?? raw.inference_model ?? prev.inference_model;
  let stt_provider = raw.sttProvider ?? raw.stt_provider ?? prev.stt_provider;
  if (
    inference_model?.startsWith('microsoft/') ||
    inference_model?.startsWith('openai/') ||
    inference_model?.includes('mai-transcribe')
  ) {
    stt_provider = 'openrouter';
  } else if (inference_model?.startsWith('groq/')) {
    stt_provider = 'groq';
  }
  const local_inference_profile = raw.localInferenceProfile ?? raw.localModelProfile ?? raw.inferenceProfile ?? raw.local_inference_profile ?? prev.local_inference_profile;
  const recording_sounds = raw.recordingSounds ?? raw.soundFeedback ?? raw.recording_sounds ?? prev.recording_sounds;
  const sound_volume = raw.soundVolume ?? raw.sound_volume ?? prev.sound_volume;
  const pause_other_audio_while_talking = raw.pauseOtherAudioWhileTalking ?? raw.pause_other_audio_while_talking ?? prev.pause_other_audio_while_talking;
  const auto_enhance_prompt = raw.autoEnhancePrompt ?? raw.auto_enhance_prompt ?? prev.auto_enhance_prompt;
  const context_aware_formatting = raw.contextAwareFormatting ?? raw.context_aware_formatting ?? prev.context_aware_formatting;
  const formatting_instructions = raw.formattingInstructions ?? raw.formatting_instructions ?? prev.formatting_instructions;
  const theme = raw.theme ?? prev.theme;
  const onboarding_completed = raw.onboardingCompleted ?? raw.onboarding_completed ?? prev.onboarding_completed;
  const mic_boost = raw.micBoost ?? raw.mic_boost ?? prev.mic_boost;
  const voice_commands_enabled = raw.voiceCommandsEnabled ?? raw.voice_commands_enabled ?? prev.voice_commands_enabled;
  const voice_snippets_enabled = raw.voiceSnippetsEnabled ?? raw.voice_snippets_enabled ?? prev.voice_snippets_enabled;
  const quick_actions_enabled = raw.quickActionsEnabled ?? raw.quick_actions_enabled ?? prev.quick_actions_enabled;
  const clipboard_history_enabled = raw.clipboardHistoryEnabled ?? raw.clipboard_history_enabled ?? prev.clipboard_history_enabled;
  const clipboard_max_items = raw.clipboardMaxItems ?? raw.clipboard_max_items ?? prev.clipboard_max_items;
  const tenglish_mode_enabled = raw.tenglishModeEnabled ?? raw.tenglish_mode_enabled ?? prev.tenglish_mode_enabled;
  const tenglish_stt_tier = raw.tenglishSttTier ?? raw.tenglish_stt_tier ?? prev.tenglish_stt_tier;
  const tenglish_translit_engine = raw.tenglishTranslitEngine ?? raw.tenglish_translit_engine ?? prev.tenglish_translit_engine;
  const noise_cancellation_enabled = raw.noiseCancellationEnabled ?? raw.noise_cancellation_enabled ?? prev.noise_cancellation_enabled;
  const ambient_memory_enabled = raw.ambientMemoryEnabled ?? raw.ambient_memory_enabled ?? prev.ambient_memory_enabled;
  const groq_api_key = raw.groq_api_key ?? raw.groqApiKey ?? prev.groq_api_key;
  const openrouter_api_key = raw.openrouter_api_key ?? raw.openRouterApiKey ?? prev.openrouter_api_key;

  return {
    ...prev,
    ...raw,
    push_to_talk_key,
    push_to_talk_mode,
    push_to_talk_combo,
    toggle_recording_key,
    microphone_device_id,
    widget_visibility,
    widget_style,
    follow_cursor_across_monitors,
    lock_widget_position,
    copy_instead_of_type,
    launch_on_startup,
    launch_at_login,
    transcription_mode,
    stt_provider,
    inference_model,
    local_inference_profile,
    recording_sounds,
    sound_volume,
    pause_other_audio_while_talking,
    auto_enhance_prompt,
    context_aware_formatting,
    formatting_instructions,
    theme,
    onboarding_completed,
    mic_boost,
    voice_commands_enabled,
    voice_snippets_enabled,
    quick_actions_enabled,
    clipboard_history_enabled,
    clipboard_max_items,
    tenglish_mode_enabled,
    tenglish_stt_tier,
    tenglish_translit_engine,
    noise_cancellation_enabled,
    ambient_memory_enabled,
    groq_api_key,
    openrouter_api_key,
    // Also include camelCase mirrors
    pushToTalkKey: push_to_talk_key,
    pushToTalkMode: push_to_talk_mode,
    pushToTalkCombo: push_to_talk_combo,
    toggleRecordingKey: toggle_recording_key,
    microphoneDeviceId: microphone_device_id,
    widgetVisibility: widget_visibility,
    widgetStyle: widget_style,
    followCursorAcrossMonitors: follow_cursor_across_monitors,
    lockWidgetPosition: lock_widget_position,
    copyInsteadOfType: copy_instead_of_type,
    pasteDirectly: !copy_instead_of_type,
    launchOnStartup: launch_on_startup,
    launchAtLogin: launch_at_login,
    transcriptionMode: transcription_mode,
    sttProvider: stt_provider,
    inferenceModel: inference_model,
    cloudModel: inference_model,
    localInferenceProfile: local_inference_profile,
    localModelProfile: local_inference_profile,
    recordingSounds: recording_sounds,
    soundFeedback: recording_sounds,
    soundVolume: sound_volume,
    pauseOtherAudioWhileTalking: pause_other_audio_while_talking,
    autoEnhancePrompt: auto_enhance_prompt,
    formattingInstructions: formatting_instructions,
    onboardingCompleted: onboarding_completed,
    micBoost: mic_boost,
    voiceCommandsEnabled: voice_commands_enabled,
    voiceSnippetsEnabled: voice_snippets_enabled,
    quickActionsEnabled: quick_actions_enabled,
    tenglishModeEnabled: tenglish_mode_enabled,
    tenglishSttTier: tenglish_stt_tier,
    tenglishTranslitEngine: tenglish_translit_engine,
    groqApiKey: groq_api_key,
    openRouterApiKey: openrouter_api_key,
  };
}

function buildRustPatch(partial: Record<string, any>): Record<string, any> {
  const patch: Record<string, any> = { ...partial };

  if (partial.push_to_talk_key !== undefined) patch.pushToTalkKey = partial.push_to_talk_key;
  if (partial.push_to_talk_mode !== undefined) patch.pushToTalkMode = partial.push_to_talk_mode;
  if (partial.push_to_talk_combo !== undefined) patch.pushToTalkCombo = partial.push_to_talk_combo;
  if (partial.toggle_recording_key !== undefined) patch.toggleRecordingKey = partial.toggle_recording_key;
  if (partial.microphone_device_id !== undefined) patch.microphoneDeviceId = partial.microphone_device_id;
  if (partial.widget_visibility !== undefined) patch.widgetVisibility = partial.widget_visibility;
  if (partial.widget_style !== undefined) patch.widgetStyle = partial.widget_style;
  if (partial.follow_cursor_across_monitors !== undefined) patch.followCursorAcrossMonitors = partial.follow_cursor_across_monitors;
  if (partial.lock_widget_position !== undefined) patch.lockWidgetPosition = partial.lock_widget_position;
  if (partial.copy_instead_of_type !== undefined) {
    patch.copyInsteadOfType = partial.copy_instead_of_type;
    patch.pasteDirectly = !partial.copy_instead_of_type;
  }
  if (partial.launch_on_startup !== undefined) {
    patch.launchOnStartup = partial.launch_on_startup;
    patch.launchAtLogin = partial.launch_on_startup;
  }
  if (partial.launch_at_login !== undefined) {
    patch.launchAtLogin = partial.launch_at_login;
    patch.launchOnStartup = partial.launch_at_login;
  }
  if (partial.transcription_mode !== undefined) patch.transcriptionMode = partial.transcription_mode;
  if (partial.stt_provider !== undefined) patch.sttProvider = partial.stt_provider;
  if (partial.inference_model !== undefined) {
    patch.inferenceModel = partial.inference_model;
    patch.cloudModel = partial.inference_model;
    if (partial.stt_provider === undefined) {
      if (
        partial.inference_model.startsWith('microsoft/') ||
        partial.inference_model.startsWith('openai/') ||
        partial.inference_model.includes('mai-transcribe')
      ) {
        patch.sttProvider = 'openrouter';
      } else if (partial.inference_model.startsWith('groq/')) {
        patch.sttProvider = 'groq';
      }
    }
  }
  if (partial.local_inference_profile !== undefined) {
    patch.localInferenceProfile = partial.local_inference_profile;
    patch.inferenceProfile = partial.local_inference_profile;
    patch.localModelProfile = partial.local_inference_profile;
  }
  if (partial.recording_sounds !== undefined) {
    patch.recordingSounds = partial.recording_sounds;
    patch.soundFeedback = partial.recording_sounds;
  }
  if (partial.sound_volume !== undefined) patch.soundVolume = partial.sound_volume;
  if (partial.pause_other_audio_while_talking !== undefined) patch.pauseOtherAudioWhileTalking = partial.pause_other_audio_while_talking;
  if (partial.auto_enhance_prompt !== undefined) patch.autoEnhancePrompt = partial.auto_enhance_prompt;
  if (partial.formatting_instructions !== undefined) patch.formattingInstructions = partial.formatting_instructions;
  if (partial.onboarding_completed !== undefined) patch.onboardingCompleted = partial.onboarding_completed;
  if (partial.mic_boost !== undefined) patch.micBoost = partial.mic_boost;
  if (partial.voice_commands_enabled !== undefined) patch.voiceCommandsEnabled = partial.voice_commands_enabled;
  if (partial.voice_snippets_enabled !== undefined) patch.voiceSnippetsEnabled = partial.voice_snippets_enabled;
  if (partial.quick_actions_enabled !== undefined) patch.quickActionsEnabled = partial.quick_actions_enabled;
  if (partial.tenglish_mode_enabled !== undefined) patch.tenglishModeEnabled = partial.tenglish_mode_enabled;
  if (partial.tenglish_stt_tier !== undefined) patch.tenglishSttTier = partial.tenglish_stt_tier;
  if (partial.tenglish_translit_engine !== undefined) patch.tenglishTranslitEngine = partial.tenglish_translit_engine;

  return patch;
}

interface SettingsContextType {
  settings: AppSettings;
  loading: boolean;
  updateSetting: (key: string, value: any) => Promise<void>;
  updateSettings: (partial: Partial<AppSettings>) => Promise<void>;
  reloadSettings: () => Promise<void>;
  saveSettings: () => Promise<void>;
}

const SettingsContext = createContext<SettingsContextType | undefined>(undefined);

export const SettingsProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [settings, setSettings] = useState<AppSettings>(defaultSettings);
  const [loading, setLoading] = useState(true);

  async function load() {
    try {
      // 1. Fetch settings from Rust getSettings
      const rawSettings = await invokeTauri<any>('getSettings');
      
      // 2. Fetch API keys from CredentialsProvider
      let groqKey: string | null = null;
      let orKey: string | null = null;
      try {
        groqKey = await invokeTauri<string | null>('getApiKey');
      } catch (err) {
        // Ignored in browser mock
      }
      try {
        orKey = await invokeTauri<string | null>('getOpenRouterApiKey');
      } catch (err) {
        // Ignored in browser mock
      }

      setSettings((prev) => {
        const normalized = normalizeIncomingSettings(rawSettings, prev);
        if (groqKey) {
          normalized.groq_api_key = groqKey;
          normalized.groqApiKey = groqKey;
        }
        if (orKey) {
          normalized.openrouter_api_key = orKey;
          normalized.openRouterApiKey = orKey;
        }
        return normalized;
      });
    } catch (e) {
      console.warn('[SettingsContext] Failed to load settings:', e);
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    load();
    const unlisten = listenTauriEvent('settingsChanged', (newSettings) => {
      if (newSettings) {
        setSettings((prev) => normalizeIncomingSettings(newSettings, prev));
      }
    });
    return () => unlisten();
  }, []);

  const updateSettings = async (partial: Partial<AppSettings>) => {
    setSettings((prev) => normalizeIncomingSettings(partial, prev));
    try {
      const patch = buildRustPatch(partial);
      await invokeTauri('saveSettings', { patch });

      // If groq_api_key or groqApiKey was updated, save it to CredentialsProvider
      const newGroqKey = partial.groq_api_key ?? partial.groqApiKey;
      if (typeof newGroqKey === 'string') {
        await invokeTauri('saveGroqKey', { key: newGroqKey });
      }

      // If openrouter_api_key or openRouterApiKey was updated, save it
      const newOrKey = partial.openrouter_api_key ?? partial.openRouterApiKey;
      if (typeof newOrKey === 'string') {
        await invokeTauri('saveOpenRouterKey', { key: newOrKey });
      }
    } catch (err) {
      console.error('[SettingsContext] Failed to save settings:', err);
    }
  };

  const updateSetting = async (key: string, value: any) => {
    await updateSettings({ [key]: value });
  };

  const saveSettings = async () => {
    try {
      const patch = buildRustPatch(settings);
      await invokeTauri('saveSettings', { patch });
    } catch (err) {
      console.error('[SettingsContext] Failed to save settings:', err);
    }
  };

  return (
    <SettingsContext.Provider
      value={{
        settings,
        loading,
        updateSetting,
        updateSettings,
        reloadSettings: load,
        saveSettings,
      }}
    >
      {children}
    </SettingsContext.Provider>
  );
};

export function useSettings() {
  const ctx = useContext(SettingsContext);
  if (!ctx) {
    throw new Error('useSettings must be used within a SettingsProvider');
  }
  return ctx;
}
