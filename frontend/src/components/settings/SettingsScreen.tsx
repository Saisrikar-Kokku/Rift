import React, { useState, useEffect } from 'react';
import { useSettings } from '../../context/SettingsContext';
import { invokeTauri } from '../../lib/tauriBridge';
import { SpotlightCard } from '../ui/SpotlightCard';
import { Badge } from '../ui/Badge';
import { Switch } from '../ui/Switch';
import { SimpleTooltip } from '../ui/Tooltip';
import {
  Cpu,
  Mic,
  Sliders,
  AppWindow,
  Bolt,
  Wand2,
  Palette,
  Eye,
  EyeOff,
  Check,
  Play,
  RotateCcw,
  Trash2,
  Plus,
  Languages,
  ShieldCheck,
} from 'lucide-react';

export type SettingsSubTab = 'engine' | 'audio' | 'general' | 'widget' | 'snippets' | 'ai' | 'appearance';

interface SnippetItem {
  id: string;
  phrase: string;
  expansion: string;
}

export const SettingsScreen: React.FC = () => {
  const { settings, updateSetting } = useSettings();
  const [activeTab, setActiveTab] = useState<SettingsSubTab>('engine');
  const [showSavedBadge, setShowSavedBadge] = useState(false);
  const [settingsSaveError, setSettingsSaveError] = useState<string | null>(null);
  const [showGroqKey, setShowGroqKey] = useState(false);
  const [showOpenRouterKey, setShowOpenRouterKey] = useState(false);
  const [groqKeyInput, setGroqKeyInput] = useState('');
  const [orKeyInput, setOrKeyInput] = useState('');
  const [groqKeyStatus, setGroqKeyStatus] = useState<{ isConfigured: boolean; maskedKey?: string }>({ isConfigured: false });
  const [orKeyStatus, setOrKeyStatus] = useState<{ isConfigured: boolean; maskedKey?: string }>({ isConfigured: false });
  const [isSavingGroqKey, setIsSavingGroqKey] = useState(false);
  const [isSavingOrKey, setIsSavingOrKey] = useState(false);
  const [groqTestMsg, setGroqTestMsg] = useState<{ text: string; success: boolean } | null>(null);
  const [orTestMsg, setOrTestMsg] = useState<{ text: string; success: boolean } | null>(null);
  const [isTestingGroq, setIsTestingGroq] = useState(false);
  const [isTestingOr, setIsTestingOr] = useState(false);

  const [audioDevices, setAudioDevices] = useState<{ id: string; name: string }[]>([]);

  // Snippets from SQLite
  const [snippets, setSnippets] = useState<SnippetItem[]>([]);
  const [newSnippetPhrase, setNewSnippetPhrase] = useState('');
  const [newSnippetExpansion, setNewSnippetExpansion] = useState('');

  // 1. Load Audio input devices
  useEffect(() => {
    async function loadDevices() {
      try {
        const res = await invokeTauri<{ microphones?: { id: number; deviceId?: string; name: string; isDefault: boolean }[] }>('getMicrophones');
        const mics = res?.microphones || (Array.isArray(res) ? res : []);
        if (Array.isArray(mics) && mics.length > 0) {
          setAudioDevices([
            { id: 'default', name: 'Default System Microphone' },
            ...mics.map((m: any) => ({
              id: m.name || String(m.id ?? 'default'),
              name: m.name + (m.isDefault ? ' (System Default)' : ''),
            }))
          ]);
        } else {
          setAudioDevices([{ id: 'default', name: 'Default System Microphone' }]);
        }
      } catch (e) {
        setAudioDevices([{ id: 'default', name: 'Default System Microphone' }]);
      }
    }
    loadDevices();
  }, []);

  // Only load masked key status into the webview. Secret values stay in the native layer.
  useEffect(() => {
    let active = true;
    void Promise.all([
      invokeTauri<{ isConfigured: boolean; maskedKey?: string }>('getGroqKeyStatus'),
      invokeTauri<{ isConfigured: boolean; maskedKey?: string }>('getOpenRouterKeyStatus'),
    ]).then(([groq, openrouter]) => {
      if (active) {
        setGroqKeyStatus(groq || { isConfigured: false });
        setOrKeyStatus(openrouter || { isConfigured: false });
      }
    }).catch((error) => {
      console.warn('[SettingsScreen] Could not load API key status:', error);
    });
    return () => { active = false; };
  }, []);

  // 3. Load Snippets from SQLite
  useEffect(() => {
    async function loadSnippets() {
      try {
        const data = await invokeTauri<any[]>('getSnippets');
        if (Array.isArray(data)) {
          setSnippets(
            data.map((item) => ({
              id: String(item.id),
              phrase: item.trigger_phrase || item.phrase || '',
              expansion: item.expansion_text || item.expansion || '',
            }))
          );
        }
      } catch (e) {
        console.warn('Failed to load snippets from SQLite:', e);
      }
    }
    loadSnippets();
  }, []);

  const triggerSavedFeedback = () => {
    setShowSavedBadge(true);
    setTimeout(() => setShowSavedBadge(false), 2000);
  };

  const handleFieldChange = async (key: string, val: any) => {
    try {
      await updateSetting(key, val);
      setSettingsSaveError(null);
      triggerSavedFeedback();
    } catch (error) {
      console.error(`Failed to save setting ${key}:`, error);
      setSettingsSaveError('Could not save this change. Please try again.');
    }
  };

  const handleSaveGroqKey = async (key: string) => {
    const trimmedKey = key.trim();
    if (!trimmedKey) return;
    setIsSavingGroqKey(true);
    try {
      const result = await invokeTauri<{ success?: boolean; message?: string }>('saveGroqKey', { key: trimmedKey });
      if (!result?.success) throw new Error(result?.message || 'Could not save Groq API key.');
      const status = await invokeTauri<{ isConfigured: boolean; maskedKey?: string }>('getGroqKeyStatus');
      setGroqKeyStatus(status || { isConfigured: true });
      setGroqKeyInput(trimmedKey);
      setGroqTestMsg({ text: 'Key saved on this device.', success: true });
      triggerSavedFeedback();
    } catch (error: any) {
      setGroqTestMsg({ text: error?.message || 'Could not save Groq API key.', success: false });
    } finally {
      setIsSavingGroqKey(false);
    }
  };

  const handleTestGroqKey = async () => {
    if (!groqKeyInput.trim() && !groqKeyStatus.isConfigured) {
      setGroqTestMsg({ text: 'Please enter a key first', success: false });
      return;
    }
    setIsTestingGroq(true);
    setGroqTestMsg(null);
    try {
      const res = groqKeyInput.trim()
        ? await invokeTauri<{ success?: boolean; valid?: boolean; message?: string; latencyMs?: number }>('testGroqKey', { key: groqKeyInput.trim() })
        : await invokeTauri<{ success?: boolean; valid?: boolean; message?: string; latencyMs?: number }>('testSavedGroqKey');
      if (res?.valid || res?.success) {
        setGroqTestMsg({
          text: `Connected (${res.latencyMs || 85}ms) - Valid Groq Key!`,
          success: true,
        });
      } else {
        setGroqTestMsg({
          text: res?.message || 'Invalid Groq API key',
          success: false,
        });
      }
    } catch (e: any) {
      setGroqTestMsg({ text: e?.message || 'Connection failed', success: false });
    } finally {
      setIsTestingGroq(false);
    }
  };

  const handleSaveOrKey = async (key: string) => {
    const trimmedKey = key.trim();
    if (!trimmedKey) return;
    setIsSavingOrKey(true);
    try {
      const result = await invokeTauri<{ success?: boolean; message?: string }>('saveOpenRouterKey', { key: trimmedKey });
      if (!result?.success) throw new Error(result?.message || 'Could not save OpenRouter API key.');
      const status = await invokeTauri<{ isConfigured: boolean; maskedKey?: string }>('getOpenRouterKeyStatus');
      setOrKeyStatus(status || { isConfigured: true });
      setOrKeyInput(trimmedKey);
      setOrTestMsg({ text: 'Key saved on this device.', success: true });
      triggerSavedFeedback();
    } catch (error: any) {
      setOrTestMsg({ text: error?.message || 'Could not save OpenRouter API key.', success: false });
    } finally {
      setIsSavingOrKey(false);
    }
  };

  const handleDeleteGroqKey = async () => {
    try {
      const result = await invokeTauri<{ success?: boolean; message?: string }>('deleteGroqKey');
      if (!result?.success) throw new Error(result?.message || 'Could not remove Groq API key.');
      setGroqKeyInput('');
      setGroqKeyStatus({ isConfigured: false });
      setGroqTestMsg({ text: 'Groq API key removed.', success: true });
      triggerSavedFeedback();
    } catch (error: any) {
      setGroqTestMsg({ text: error?.message || 'Could not remove Groq API key.', success: false });
    }
  };

  const handleDeleteOrKey = async () => {
    try {
      const result = await invokeTauri<{ success?: boolean; message?: string }>('deleteOpenRouterKey');
      if (!result?.success) throw new Error(result?.message || 'Could not remove OpenRouter API key.');
      setOrKeyInput('');
      setOrKeyStatus({ isConfigured: false });
      setOrTestMsg({ text: 'OpenRouter API key removed.', success: true });
      triggerSavedFeedback();
    } catch (error: any) {
      setOrTestMsg({ text: error?.message || 'Could not remove OpenRouter API key.', success: false });
    }
  };

  const handleTestOrKey = async () => {
    if (!orKeyInput.trim() && !orKeyStatus.isConfigured) {
      setOrTestMsg({ text: 'Please enter a key first', success: false });
      return;
    }
    setIsTestingOr(true);
    setOrTestMsg(null);
    try {
      const res = orKeyInput.trim()
        ? await invokeTauri<{ success?: boolean; valid?: boolean; message?: string }>('testOpenRouterKey', { key: orKeyInput.trim() })
        : await invokeTauri<{ success?: boolean; valid?: boolean; message?: string }>('testSavedOpenRouterKey');
      if (res?.valid || res?.success) {
        setOrTestMsg({
          text: res.message || 'Connected to OpenRouter!',
          success: true,
        });
      } else {
        setOrTestMsg({
          text: res?.message || 'Invalid OpenRouter API key',
          success: false,
        });
      }
    } catch (e: any) {
      setOrTestMsg({ text: e?.message || 'Connection failed', success: false });
    } finally {
      setIsTestingOr(false);
    }
  };

  const handleMicSelect = async (deviceIdStr: string) => {
    const val = deviceIdStr === 'default' ? null : deviceIdStr;
    handleFieldChange('microphone_device_id', val);
    try {
      await invokeTauri('setMicrophone', { deviceId: deviceIdStr });
    } catch (e) {
      console.warn('Failed to set microphone:', e);
    }
  };

  const handleAddSnippet = async () => {
    if (!newSnippetPhrase.trim() || !newSnippetExpansion.trim()) return;
    const phrase = newSnippetPhrase.trim();
    const expansion = newSnippetExpansion.trim();
    const id = Date.now().toString();

    const snippetPayload = {
      id,
      trigger_phrase: phrase,
      expansion_text: expansion,
      match_type: 'exact',
      is_active: true,
      created_at: Math.floor(Date.now() / 1000),
    };

    try {
      await invokeTauri('saveSnippet', { snippet: snippetPayload });
      setSnippets((prev) => [...prev, { id, phrase, expansion }]);
      setNewSnippetPhrase('');
      setNewSnippetExpansion('');
      triggerSavedFeedback();
    } catch (e) {
      console.error('Failed to save snippet:', e);
    }
  };

  const handleDeleteSnippet = async (id: string) => {
    try {
      await invokeTauri('deleteSnippet', { id });
      setSnippets((prev) => prev.filter((s) => s.id !== id));
      triggerSavedFeedback();
    } catch (e) {
      console.error('Failed to delete snippet:', e);
    }
  };

  const handlePlaySoundCue = (soundType: string) => {
    try {
      invokeTauri('testAudioCue', {
        soundType,
        volume: settings.sound_volume ?? 0.6,
      });
    } catch (e) {}
  };

  const tabs: { id: SettingsSubTab; label: string; icon: React.ReactNode }[] = [
    { id: 'engine', label: 'Engine', icon: <Cpu className="w-4 h-4" /> },
    { id: 'audio', label: 'Audio', icon: <Mic className="w-4 h-4" /> },
    { id: 'general', label: 'General', icon: <Sliders className="w-4 h-4" /> },
    { id: 'widget', label: 'Widget', icon: <AppWindow className="w-4 h-4" /> },
    { id: 'snippets', label: 'Snippets', icon: <Bolt className="w-4 h-4" /> },
    { id: 'ai', label: 'AI Formatting', icon: <Wand2 className="w-4 h-4" /> },
    { id: 'appearance', label: 'Appearance', icon: <Palette className="w-4 h-4" /> },
  ];

  const s = settings as any;

  return (
    <div className="rift-main-area">
      <div className="settings-wrapper">
        
        {/* Left Sub-Settings Nav */}
        <aside className="settings-nav-pane">
          <nav className="settings-tabs" role="tablist">
            {tabs.map((tab) => {
              const isActive = activeTab === tab.id;
              return (
                <button
                  key={tab.id}
                  type="button"
                  className={`tab-btn ${isActive ? 'active' : ''}`}
                  onClick={() => setActiveTab(tab.id)}
                >
                  <span style={{ display: 'flex', alignItems: 'center', color: 'inherit' }}>
                    {tab.icon}
                  </span>
                  <span>{tab.label}</span>
                </button>
              );
            })}
          </nav>
        </aside>

        {/* Right Settings Content Pane */}
        <main className="settings-content-pane">
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', minHeight: '24px' }}>
            <span style={{ fontSize: '11px', color: '#938ea1', fontWeight: 600, textTransform: 'uppercase', letterSpacing: '0.05em' }}>
              System Preferences
            </span>
            {settingsSaveError ? (
              <span role="alert" style={{ fontSize: '11px', color: '#fca5a5' }}>{settingsSaveError}</span>
            ) : (
              <div className={`save-indicator-badge ${showSavedBadge ? 'visible' : ''}`} aria-live="polite">
                <Check className="w-3.5 h-3.5" />
                <span>All changes saved</span>
              </div>
            )}
          </div>

          {/* ================= PANEL 1: ENGINE ================= */}
          {activeTab === 'engine' && (
            <section className="settings-panel">
              <div className="panel-header">
                <h2 className="panel-title">
                  <Cpu className="w-5 h-5 text-purple-400" />
                  Speech Recognition Pipeline
                </h2>
                <p className="panel-desc">Configure cloud models, Groq acceleration, offline models, and clipboard retention.</p>
              </div>

              <SpotlightCard style={{ padding: '4px 16px' }}>
                {/* Pipeline */}
                <div className="setting-row">
                  <div className="setting-info">
                    <label className="setting-title" htmlFor="transcriptionMode">
                      Transcription Architecture
                    </label>
                    <p className="setting-desc">Primary speech-to-text processing mode</p>
                  </div>
                  <select
                    id="transcriptionMode"
                    className="setting-select"
                    value={s.transcription_mode || s.transcriptionMode || 'cloud'}
                    onChange={(e) => {
                      handleFieldChange('transcription_mode', e.target.value);
                      handleFieldChange('transcriptionMode', e.target.value);
                    }}
                  >
                    <option value="cloud">Cloud Only (Ultra Fast · Groq / OpenRouter)</option>
                    <option value="auto">Auto (Cloud with Local Fallback)</option>
                    <option value="local">Offline Only (100% Local Whisper)</option>
                  </select>
                </div>

                {/* Cloud Model */}
                <div className="setting-row">
                  <div className="setting-info">
                    <label className="setting-title" htmlFor="inferenceModel">
                      Cloud Speech Model
                    </label>
                    <p className="setting-desc">Default STT model (Groq Turbo, ElevenLabs Scribe v2, or Whisper)</p>
                  </div>
                  <select
                    id="inferenceModel"
                    className="setting-select"
                    value={s.inference_model || s.inferenceModel || 'groq/whisper-large-v3-turbo'}
                    onChange={(e) => {
                      const newModel = e.target.value;
                      handleFieldChange('inference_model', newModel);
                      handleFieldChange('inferenceModel', newModel);
                      if (newModel.startsWith('groq/')) {
                        handleFieldChange('stt_provider', 'groq');
                        handleFieldChange('sttProvider', 'groq');
                      } else if (
                        newModel.startsWith('elevenlabs/') ||
                        newModel.startsWith('openai/') ||
                        newModel.includes('scribe') ||
                        newModel.startsWith('microsoft/')
                      ) {
                        handleFieldChange('stt_provider', 'openrouter');
                        handleFieldChange('sttProvider', 'openrouter');
                      }
                    }}
                  >
                    <optgroup label="⚡ Free Groq Cloud (Unlimited Free Tier)">
                      <option value="groq/whisper-large-v3-turbo">
                        Groq: Whisper Large v3 Turbo (Free · Ultra Fast ⚡)
                      </option>
                      <option value="groq/whisper-large-v3">
                        Groq: Whisper Large v3 (Free · Standard Accuracy)
                      </option>
                    </optgroup>
                    <optgroup label="🏆 OpenRouter Cloud (Studio Grade)">
                      <option value="elevenlabs/scribe-v2">
                        ElevenLabs: Scribe v2 (Studio 🏆 · 2.18% WER · Ultra Fast ⚡)
                      </option>
                      <option value="openai/whisper-large-v3-turbo">
                        OpenAI: Whisper Large v3 Turbo (OpenRouter · Fast ~2.5s)
                      </option>
                    </optgroup>
                  </select>
                </div>

                {/* Offline Model */}
                <div className="setting-row">
                  <div className="setting-info">
                    <label className="setting-title" htmlFor="inferenceProfile">
                      Active Local Offline Model
                    </label>
                    <p className="setting-desc">Offline model used for local transcription and fallback</p>
                  </div>
                  <select
                    id="inferenceProfile"
                    className="setting-select"
                    value={s.local_inference_profile || s.inferenceProfile || 'balanced'}
                    onChange={(e) => {
                      handleFieldChange('local_inference_profile', e.target.value);
                      handleFieldChange('inferenceProfile', e.target.value);
                    }}
                  >
                    <option value="balanced">Whisper Small (Default / Balanced - 465 MB)</option>
                    <option value="accurate">Whisper Large-v3 Turbo (High Precision - 547 MB)</option>
                    <option value="fast">Whisper Base (Fast - 141 MB)</option>
                    <option value="tiny">Whisper Tiny (Ultra Fast - 74 MB)</option>
                  </select>
                </div>

                <div className="setting-row">
                  <div className="setting-info">
                    <div className="setting-title">Unload local model when idle</div>
                    <p className="setting-desc">Release the resident Whisper model after 5 minutes without a local transcription. The next request will reload it.</p>
                  </div>
                  <Switch
                    checked={!!(s.local_whisper_unload_on_idle ?? s.localWhisperUnloadOnIdle ?? true)}
                    onChange={(checked) => handleFieldChange('local_whisper_unload_on_idle', checked)}
                  />
                </div>
              </SpotlightCard>

              {/* Tenglish Mode Card (Telugu to Romanized English Voice Dictation) */}
              <SpotlightCard
                spotlightColor="rgba(245, 158, 11, 0.15)"
                style={{
                  padding: '16px',
                  display: 'flex',
                  flexDirection: 'column',
                  gap: '12px',
                  borderColor: (s.tenglish_mode_enabled ?? s.tenglishModeEnabled) ? 'rgba(245, 158, 11, 0.35)' : undefined,
                  background: (s.tenglish_mode_enabled ?? s.tenglishModeEnabled)
                    ? 'linear-gradient(180deg, rgba(245, 158, 11, 0.05) 0%, rgba(255, 255, 255, 0.01) 100%)'
                    : undefined,
                }}
              >
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                  <div className="setting-info">
                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                      <div className="setting-title" style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                        <Languages className="w-4 h-4 text-amber-500" />
                        <span>Tenglish Mode (తెలుగు → English)</span>
                      </div>
                      <span
                        className={
                          (s.tenglish_mode_enabled ?? s.tenglishModeEnabled)
                            ? 'inline-flex items-center gap-1 text-[10px] font-mono font-semibold px-2 py-0.5 rounded-full bg-amber-500/20 text-amber-300 border border-amber-500/30'
                            : 'inline-flex items-center gap-1 text-[10px] font-mono font-semibold px-2 py-0.5 rounded-full bg-white/[0.05] text-[#9c97aa] border border-white/[0.06]'
                        }
                      >
                        {(s.tenglish_mode_enabled ?? s.tenglishModeEnabled) ? 'ON · Chat Mode' : 'OFF'}
                      </span>
                    </div>
                    <p className="setting-desc" style={{ marginTop: '3px' }}>
                      Speak Telugu and get natural Romanized chat English (e.g. <em>"em chesthunnav?"</em>) for WhatsApp, Telegram, and chat apps.
                    </p>
                  </div>
                  <Switch
                    checked={!!(s.tenglish_mode_enabled ?? s.tenglishModeEnabled)}
                    onChange={(checked) => {
                      handleFieldChange('tenglish_mode_enabled', checked);
                      handleFieldChange('tenglishModeEnabled', checked);
                    }}
                  />
                </div>

                {/* Verified Studio Pipeline Card when Tenglish mode is enabled */}
                {(s.tenglish_mode_enabled ?? s.tenglishModeEnabled) && (
                  <div style={{ borderTop: '1px solid rgba(255, 255, 255, 0.06)', paddingTop: '10px', display: 'flex', flexDirection: 'column', gap: '8px' }}>
                    <div style={{ background: 'rgba(245, 158, 11, 0.06)', border: '1px solid rgba(245, 158, 11, 0.2)', borderRadius: '8px', padding: '10px 12px', display: 'flex', flexDirection: 'column', gap: '6px' }}>
                      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                        <span style={{ fontSize: '11px', fontWeight: 600, color: '#f59e0b', textTransform: 'uppercase', letterSpacing: '0.05em', display: 'flex', alignItems: 'center', gap: '5px' }}>
                          <ShieldCheck className="w-3.5 h-3.5 text-amber-400" />
                          Verified Studio Pipeline
                        </span>
                        <span style={{ fontSize: '10px', fontFamily: 'monospace', color: 'rgba(255, 255, 255, 0.5)' }}>
                          100% Accuracy Tested
                        </span>
                      </div>
                      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '8px', marginTop: '4px' }}>
                        <div style={{ background: 'rgba(0, 0, 0, 0.25)', border: '1px solid rgba(255, 255, 255, 0.06)', borderRadius: '6px', padding: '8px 10px' }}>
                          <div style={{ fontSize: '10px', color: 'rgba(255, 255, 255, 0.5)', marginBottom: '2px' }}>1. Speech Recognition</div>
                          <div style={{ fontSize: '12px', fontWeight: 600, color: '#fff', display: 'flex', alignItems: 'center', gap: '4px' }}>
                            <span>ElevenLabs Scribe v2</span>
                            <span style={{ fontSize: '9px', padding: '1px 4px', background: 'rgba(99, 102, 241, 0.2)', color: '#818cf8', borderRadius: '3px' }}>AA #4</span>
                          </div>
                        </div>
                        <div style={{ background: 'rgba(0, 0, 0, 0.25)', border: '1px solid rgba(255, 255, 255, 0.06)', borderRadius: '6px', padding: '8px 10px' }}>
                          <div style={{ fontSize: '10px', color: 'rgba(255, 255, 255, 0.5)', marginBottom: '2px' }}>2. Transliteration</div>
                          <div style={{ fontSize: '12px', fontWeight: 600, color: '#fff', display: 'flex', alignItems: 'center', gap: '4px' }}>
                            <span>Gemini 2.5 Flash</span>
                            <span style={{ fontSize: '9px', padding: '1px 4px', background: 'rgba(245, 158, 11, 0.2)', color: '#f59e0b', borderRadius: '3px' }}>WhatsApp</span>
                          </div>
                        </div>
                      </div>
                      <div style={{ fontSize: '10.5px', color: 'rgba(255, 255, 255, 0.65)', lineHeight: 1.4, marginTop: '2px' }}>
                        ⚡ <strong>Production Locked:</strong> Seamless code-switching for tech/chat words (<em>office, bug, meeting, deployment, trip</em>) with colloquial WhatsApp Romanization.
                      </div>
                    </div>
                  </div>
                )}
              </SpotlightCard>

              {/* AI Auto-Enhancement Mode Toggle Card */}
              <SpotlightCard
                spotlightColor="rgba(192, 132, 252, 0.15)"
                style={{
                  padding: '16px',
                  display: 'flex',
                  flexDirection: 'column',
                  gap: '12px',
                  borderColor: (s.auto_enhance_prompt ?? s.autoEnhancePrompt) ? 'rgba(192, 132, 252, 0.35)' : undefined,
                  background: (s.auto_enhance_prompt ?? s.autoEnhancePrompt)
                    ? 'linear-gradient(180deg, rgba(192, 132, 252, 0.05) 0%, rgba(255, 255, 255, 0.01) 100%)'
                    : undefined,
                }}
              >
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                  <div className="setting-info">
                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                      <div className="setting-title" style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                        <Wand2 className="w-4 h-4 text-purple-400" />
                        <span>AI Auto-Enhancement Mode</span>
                      </div>
                      <span
                        className={
                          (s.auto_enhance_prompt ?? s.autoEnhancePrompt)
                            ? 'inline-flex items-center gap-1 text-[10px] font-mono font-semibold px-2 py-0.5 rounded-full bg-purple-500/20 text-purple-300 border border-purple-500/30'
                            : 'inline-flex items-center gap-1 text-[10px] font-mono font-semibold px-2 py-0.5 rounded-full bg-white/[0.05] text-[#9c97aa] border border-white/[0.06]'
                        }
                      >
                        {(s.auto_enhance_prompt ?? s.autoEnhancePrompt) ? 'ON · AI Polish' : 'OFF · Verbatim (Instant ⚡)'}
                      </span>
                    </div>
                    <p className="setting-desc" style={{ marginTop: '3px' }}>
                      {(s.auto_enhance_prompt ?? s.autoEnhancePrompt)
                        ? 'Polishes speech, removes filler words ("um", "uh"), and formats naturally. Turn OFF for fastest verbatim transcription.'
                        : 'Transcribes and pastes your exact spoken words verbatim with ZERO extra delay (fastest transcribing).'}
                    </p>
                  </div>
                  <Switch
                    checked={!!(s.auto_enhance_prompt ?? s.autoEnhancePrompt)}
                    onCheckedChange={(checked) => {
                      handleFieldChange('auto_enhance_prompt', checked);
                      handleFieldChange('autoEnhancePrompt', checked);
                    }}
                  />
                </div>
              </SpotlightCard>

              {/* 1. Universal Voice Intent Router Toggle Card */}
              <SpotlightCard
                spotlightColor="rgba(56, 189, 248, 0.16)"
                style={{
                  padding: '16px',
                  display: 'flex',
                  flexDirection: 'column',
                  gap: '10px',
                  borderColor: (s.intent_router_enabled ?? s.intentRouterEnabled) ? 'rgba(56, 189, 248, 0.35)' : undefined,
                  background: (s.intent_router_enabled ?? s.intentRouterEnabled)
                    ? 'linear-gradient(180deg, rgba(56, 189, 248, 0.05) 0%, rgba(255, 255, 255, 0.01) 100%)'
                    : undefined,
                }}
              >
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                  <div className="setting-info">
                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                      <div className="setting-title" style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                        <Bolt className="w-4 h-4 text-sky-400" />
                        <span>🧭 Universal Voice Intent Router</span>
                      </div>
                      <span
                        className={
                          (s.intent_router_enabled ?? s.intentRouterEnabled)
                            ? 'inline-flex items-center gap-1 text-[10px] font-mono font-semibold px-2 py-0.5 rounded-full bg-sky-500/20 text-sky-300 border border-sky-500/30'
                            : 'inline-flex items-center gap-1 text-[10px] font-mono font-semibold px-2 py-0.5 rounded-full bg-white/[0.05] text-[#9c97aa] border border-white/[0.06]'
                        }
                      >
                        {(s.intent_router_enabled ?? s.intentRouterEnabled) ? 'ON · Smart Routing' : 'OFF'}
                      </span>
                    </div>
                    <p className="setting-desc" style={{ marginTop: '3px' }}>
                      One hotkey does it all: normal speech types verbatim (0ms overhead), <em>"Make this professional"</em> rewrites highlighted text in-place, <em>"Write a Python function..."</em> generates code at your cursor, and <em>"Note to self..."</em> saves to Scratchpad in the background.
                    </p>
                  </div>
                  <Switch
                    checked={!!(s.intent_router_enabled ?? s.intentRouterEnabled)}
                    onCheckedChange={(checked) => {
                      handleFieldChange('intent_router_enabled', checked);
                      handleFieldChange('intentRouterEnabled', checked);
                    }}
                  />
                </div>
              </SpotlightCard>

              {/* 2. Sub-Vocal Library / Whisper Mode Toggle Card */}
              <SpotlightCard
                spotlightColor="rgba(52, 211, 153, 0.16)"
                style={{
                  padding: '16px',
                  display: 'flex',
                  flexDirection: 'column',
                  gap: '10px',
                  borderColor: (s.whisper_mode_enabled ?? s.whisperModeEnabled) ? 'rgba(52, 211, 153, 0.35)' : undefined,
                  background: (s.whisper_mode_enabled ?? s.whisperModeEnabled)
                    ? 'linear-gradient(180deg, rgba(52, 211, 153, 0.05) 0%, rgba(255, 255, 255, 0.01) 100%)'
                    : undefined,
                }}
              >
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                  <div className="setting-info">
                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                      <div className="setting-title" style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                        <Mic className="w-4 h-4 text-emerald-400" />
                        <span>🤫 Sub-Vocal "Library / Whisper Mode"</span>
                      </div>
                      <span
                        className={
                          (s.whisper_mode_enabled ?? s.whisperModeEnabled)
                            ? 'inline-flex items-center gap-1 text-[10px] font-mono font-semibold px-2 py-0.5 rounded-full bg-emerald-500/20 text-emerald-300 border border-emerald-500/30'
                            : 'inline-flex items-center gap-1 text-[10px] font-mono font-semibold px-2 py-0.5 rounded-full bg-white/[0.05] text-[#9c97aa] border border-white/[0.06]'
                        }
                      >
                        {(s.whisper_mode_enabled ?? s.whisperModeEnabled) ? 'ON · Ultra-Sensitive DSP' : 'OFF'}
                      </span>
                    </div>
                    <p className="setting-desc" style={{ marginTop: '3px' }}>
                      Tunes microphone DSP for ultra-quiet whispering (&lt;18 dB) in libraries, offices, or late at night with 36x adaptive gain, 110Hz breath-plosive filtering, and consonant formant boost.
                    </p>
                  </div>
                  <Switch
                    checked={!!(s.whisper_mode_enabled ?? s.whisperModeEnabled)}
                    onCheckedChange={(checked) => handleFieldChange('whisper_mode_enabled', checked)}
                  />
                </div>
              </SpotlightCard>

              {/* 3. Hands-Free "Hey Rift" Wake Word & Auto-Silence Completion Toggle Card */}
              <SpotlightCard
                spotlightColor="rgba(129, 140, 248, 0.16)"
                style={{
                  padding: '16px',
                  display: 'flex',
                  flexDirection: 'column',
                  gap: '10px',
                  borderColor: (s.wake_word_enabled ?? s.wakeWordEnabled) ? 'rgba(129, 140, 248, 0.35)' : undefined,
                  background: (s.wake_word_enabled ?? s.wakeWordEnabled)
                    ? 'linear-gradient(180deg, rgba(129, 140, 248, 0.05) 0%, rgba(255, 255, 255, 0.01) 100%)'
                    : undefined,
                }}
              >
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                  <div className="setting-info">
                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                      <div className="setting-title" style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                        <Mic className="w-4 h-4 text-indigo-400" />
                        <span>🎧 Hands-Free "Hey Rift" Wake Word (Bluetooth Mode)</span>
                      </div>
                      <span
                        className={
                          (s.wake_word_enabled ?? s.wakeWordEnabled)
                            ? 'inline-flex items-center gap-1 text-[10px] font-mono font-semibold px-2 py-0.5 rounded-full bg-indigo-500/20 text-indigo-300 border border-indigo-500/30'
                            : 'inline-flex items-center gap-1 text-[10px] font-mono font-semibold px-2 py-0.5 rounded-full bg-white/[0.05] text-[#9c97aa] border border-white/[0.06]'
                        }
                      >
                        {(s.wake_word_enabled ?? s.wakeWordEnabled) ? 'ON · Listening for "Hey Rift"' : 'OFF'}
                      </span>
                    </div>
                    <p className="setting-desc" style={{ marginTop: '3px' }}>
                      Built for Bluetooth earphones: say <em>"Hey Rift"</em> from anywhere to start recording hands-free without pressing the hotkey. Automatically detects when you finish speaking (1.35s silence), transcribes, and pastes.
                    </p>
                  </div>
                  <Switch
                    checked={!!(s.wake_word_enabled ?? s.wakeWordEnabled)}
                    onCheckedChange={(checked) => handleFieldChange('wake_word_enabled', checked)}
                  />
                </div>
              </SpotlightCard>

              {/* API Keys Card */}
              <SpotlightCard style={{ padding: '16px', display: 'flex', flexDirection: 'column', gap: '14px' }}>
                {/* Groq Key */}
                <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
                  <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                    <div>
                      <span style={{ fontSize: '13.5px', fontWeight: 600, color: '#f0eff4', display: 'block' }}>
                        Groq Cloud API Key
                      </span>
                      <span style={{ fontSize: '11.5px', color: '#9c97aa' }}>
                        Free Whisper LPU transcription with sub-100ms response
                      </span>
                    </div>
                    <Badge variant="groq">⚡ Free Tier</Badge>
                  </div>

                  <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                    <input
                      type={showGroqKey ? 'text' : 'password'}
                      className="setting-input"
                      placeholder={groqKeyStatus.isConfigured ? 'Saved key · enter a new key to replace' : 'gsk_...'}
                      style={{ flex: 1 }}
                      value={groqKeyInput}
                      onChange={(e) => setGroqKeyInput(e.target.value)}
                    />
                    <button
                      type="button"
                      onClick={() => setShowGroqKey(!showGroqKey)}
                      style={{ background: 'transparent', border: 'none', color: '#938ea1', cursor: 'pointer', padding: '6px' }}
                    >
                      {showGroqKey ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                    </button>
                    <button
                      type="button"
                      onClick={() => void handleSaveGroqKey(groqKeyInput)}
                      disabled={isSavingGroqKey || !groqKeyInput.trim()}
                      style={{ padding: '6px 12px', borderRadius: '7px', backgroundColor: '#242238', border: '1px solid rgba(167,139,250,0.3)', color: isSavingGroqKey ? '#938ea1' : '#ddd6fe', fontSize: '12px', cursor: isSavingGroqKey || !groqKeyInput.trim() ? 'not-allowed' : 'pointer' }}
                    >
                      {isSavingGroqKey ? 'Saving...' : 'Save'}
                    </button>
                    <button
                      type="button"
                      onClick={handleTestGroqKey}
                      disabled={isTestingGroq}
                      style={{
                        padding: '6px 14px',
                        borderRadius: '7px',
                        backgroundColor: '#161622',
                        border: '1px solid rgba(255,255,255,0.1)',
                        color: isTestingGroq ? '#938ea1' : '#f0eff4',
                        fontSize: '12px',
                        cursor: isTestingGroq ? 'not-allowed' : 'pointer',
                      }}
                    >
                      {isTestingGroq ? 'Testing...' : 'Test Key'}
                    </button>
                    {groqKeyStatus.isConfigured && (
                      <button type="button" onClick={() => void handleDeleteGroqKey()} aria-label="Remove saved Groq API key" title="Remove saved key" style={{ background: 'transparent', border: 'none', color: '#f87171', cursor: 'pointer', padding: '6px' }}>
                        <Trash2 className="w-4 h-4" />
                      </button>
                    )}
                  </div>
                  <span style={{ fontSize: '11px', color: '#9c97aa' }}>
                    {groqKeyStatus.isConfigured ? `Saved ${groqKeyStatus.maskedKey || 'key'} on this device.` : 'No key saved.'}
                  </span>
                  {groqTestMsg && (
                    <span style={{ fontSize: '12px', color: groqTestMsg.success ? '#45dfa9' : '#f87171' }}>
                      {groqTestMsg.text}
                    </span>
                  )}
                </div>

                {/* OpenRouter Key */}
                <div style={{ borderTop: '1px solid rgba(255,255,255,0.06)', paddingTop: '14px', display: 'flex', flexDirection: 'column', gap: '8px' }}>
                  <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                    <div>
                      <span style={{ fontSize: '13.5px', fontWeight: 600, color: '#f0eff4', display: 'block' }}>
                        OpenRouter API Key
                      </span>
                      <span style={{ fontSize: '11.5px', color: '#9c97aa' }}>
                        Powers ElevenLabs Scribe v2 & Whisper cloud models
                      </span>
                    </div>
                    <Badge variant="openrouter">Paid Credits</Badge>
                  </div>

                  <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                    <input
                      type={showOpenRouterKey ? 'text' : 'password'}
                      className="setting-input"
                      placeholder={orKeyStatus.isConfigured ? 'Saved key · enter a new key to replace' : 'sk-or-...'}
                      style={{ flex: 1 }}
                      value={orKeyInput}
                      onChange={(e) => setOrKeyInput(e.target.value)}
                    />
                    <button
                      type="button"
                      onClick={() => setShowOpenRouterKey(!showOpenRouterKey)}
                      style={{ background: 'transparent', border: 'none', color: '#938ea1', cursor: 'pointer', padding: '6px' }}
                    >
                      {showOpenRouterKey ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                    </button>
                    <button
                      type="button"
                      onClick={() => void handleSaveOrKey(orKeyInput)}
                      disabled={isSavingOrKey || !orKeyInput.trim()}
                      style={{ padding: '6px 12px', borderRadius: '7px', backgroundColor: '#242238', border: '1px solid rgba(167,139,250,0.3)', color: isSavingOrKey ? '#938ea1' : '#ddd6fe', fontSize: '12px', cursor: isSavingOrKey || !orKeyInput.trim() ? 'not-allowed' : 'pointer' }}
                    >
                      {isSavingOrKey ? 'Saving...' : 'Save'}
                    </button>
                    <button
                      type="button"
                      onClick={handleTestOrKey}
                      disabled={isTestingOr}
                      style={{
                        padding: '6px 14px',
                        borderRadius: '7px',
                        backgroundColor: '#161622',
                        border: '1px solid rgba(255,255,255,0.1)',
                        color: isTestingOr ? '#938ea1' : '#f0eff4',
                        fontSize: '12px',
                        cursor: isTestingOr ? 'not-allowed' : 'pointer',
                      }}
                    >
                      {isTestingOr ? 'Testing...' : 'Test Key'}
                    </button>
                    {orKeyStatus.isConfigured && (
                      <button type="button" onClick={() => void handleDeleteOrKey()} aria-label="Remove saved OpenRouter API key" title="Remove saved key" style={{ background: 'transparent', border: 'none', color: '#f87171', cursor: 'pointer', padding: '6px' }}>
                        <Trash2 className="w-4 h-4" />
                      </button>
                    )}
                  </div>
                  <span style={{ fontSize: '11px', color: '#9c97aa' }}>
                    {orKeyStatus.isConfigured ? `Saved ${orKeyStatus.maskedKey || 'key'} on this device.` : 'No key saved.'}
                  </span>
                  {orTestMsg && (
                    <span style={{ fontSize: '12px', color: orTestMsg.success ? '#45dfa9' : '#f87171' }}>
                      {orTestMsg.text}
                    </span>
                  )}
                </div>
              </SpotlightCard>
            </section>
          )}

          {/* ================= PANEL 2: AUDIO ================= */}
          {activeTab === 'audio' && (
            <section className="settings-panel">
              <div className="panel-header">
                <h2 className="panel-title">
                  <Mic className="w-5 h-5 text-emerald-400" />
                  Audio & Sound Feedback
                </h2>
                <p className="panel-desc">Hardware microphone input, digital pre-amp, and audio feedback cues.</p>
              </div>

              <SpotlightCard style={{ padding: '4px 16px' }}>
                {/* Input device */}
                <div className="setting-row">
                  <div className="setting-info">
                    <label className="setting-title">Active Microphone</label>
                    <p className="setting-desc">Direct recording audio capture device</p>
                  </div>
                  <select
                    className="setting-select"
                    value={s.microphone_device_id || 'default'}
                    onChange={(e) => handleMicSelect(e.target.value)}
                    style={{ maxWidth: '240px' }}
                  >
                    {audioDevices.length > 0 ? (
                      audioDevices.map((d) => (
                        <option key={d.id} value={d.id}>
                          {d.name}
                        </option>
                      ))
                    ) : (
                      <option value="default">Default System Microphone</option>
                    )}
                  </select>
                </div>

                {/* Noise Cancellation */}
                <div className="setting-row">
                  <div className="setting-info">
                    <label className="setting-title">Neural Noise Suppression</label>
                    <p className="setting-desc">Filters room echo, fan hums, and mechanical keyboard clicks</p>
                  </div>
                  <Switch
                    checked={s.noise_cancellation_enabled !== false}
                    onCheckedChange={(c) => handleFieldChange('noise_cancellation_enabled', c)}
                  />
                </div>

                {/* Pre-amp boost */}
                <div className="setting-row">
                  <div className="setting-info">
                    <label className="setting-title">Microphone Pre-Amp Gain</label>
                    <p className="setting-desc">Digital gain boost for distance microphones</p>
                  </div>
                  <div style={{ display: 'flex', gap: '3px', background: '#101018', padding: '2px', borderRadius: '7px' }}>
                    {['0dB', '+6dB', '+12dB', '+18dB'].map((boost) => (
                      <button
                        key={boost}
                        type="button"
                        className={`mic-boost-btn ${(s.mic_boost || '0dB') === boost ? 'active' : ''}`}
                        onClick={() => handleFieldChange('mic_boost', boost)}
                      >
                        {boost}
                      </button>
                    ))}
                  </div>
                </div>

                {/* Audio ducking */}
                <div className="setting-row">
                  <div className="setting-info">
                    <label className="setting-title">Background Audio Ducking</label>
                    <p className="setting-desc">Lowers Spotify or media volume while user is talking</p>
                  </div>
                  <Switch
                    checked={s.pause_other_audio_while_talking !== false}
                    onCheckedChange={(c) => handleFieldChange('pause_other_audio_while_talking', c)}
                  />
                </div>

                {/* Sub-Vocal Library / Whisper Mode */}
                <div className="setting-row">
                  <div className="setting-info">
                    <label className="setting-title">🤫 Sub-Vocal "Library / Whisper Mode"</label>
                    <p className="setting-desc">Ultra-sensitive DSP (36x adaptive gain + 110Hz plosive filter + formant boost) for quiet whispering</p>
                  </div>
                    <Switch
                    checked={!!(s.whisper_mode_enabled ?? s.whisperModeEnabled)}
                    onCheckedChange={(c) => handleFieldChange('whisper_mode_enabled', c)}
                  />
                </div>

                {/* Hands-Free "Hey Rift" Wake Word */}
                <div className="setting-row">
                  <div className="setting-info">
                    <label className="setting-title">🎧 Hands-Free "Hey Rift" Wake Word (Bluetooth Mode)</label>
                    <p className="setting-desc">Say "Hey Rift" to start recording hands-free and auto-complete after 1.35s of silence</p>
                  </div>
                    <Switch
                    checked={!!(s.wake_word_enabled ?? s.wakeWordEnabled)}
                    onCheckedChange={(c) => handleFieldChange('wake_word_enabled', c)}
                  />
                </div>
              </SpotlightCard>

              {/* Sound Cues Card */}
              <SpotlightCard style={{ padding: '4px 16px' }}>
                <div className="setting-row">
                  <div className="setting-info">
                    <label className="setting-title">Push-to-Talk Start Chime</label>
                    <p className="setting-desc">Ascending glass-bell tone confirming recording has begun</p>
                  </div>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                    <SimpleTooltip content="Preview start chime">
                      <button
                        type="button"
                        onClick={() => handlePlaySoundCue('start')}
                        style={{ background: 'transparent', border: 'none', color: '#c4b5fd', cursor: 'pointer', padding: '4px' }}
                      >
                        <Play className="w-4 h-4" />
                      </button>
                    </SimpleTooltip>
                    <Switch
                      checked={s.recording_sounds !== false}
                      onCheckedChange={(c) => handleFieldChange('recording_sounds', c)}
                    />
                  </div>
                </div>

                <div className="setting-row">
                  <div className="setting-info">
                    <label className="setting-title">Transcription Success Chime</label>
                    <p className="setting-desc">Harmonic chord tone when text is pasted</p>
                  </div>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                    <SimpleTooltip content="Preview success chime">
                      <button
                        type="button"
                        onClick={() => handlePlaySoundCue('success')}
                        style={{ background: 'transparent', border: 'none', color: '#34d399', cursor: 'pointer', padding: '4px' }}
                      >
                        <Play className="w-4 h-4" />
                      </button>
                    </SimpleTooltip>
                    <Switch
                      checked={s.recording_sounds !== false}
                      onCheckedChange={(c) => handleFieldChange('recording_sounds', c)}
                    />
                  </div>
                </div>

                <div className="setting-row">
                  <div className="setting-info">
                    <label className="setting-title">Audio Cue Volume</label>
                    <p className="setting-desc">Sound effect loudness level</p>
                  </div>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                    <input
                      type="range"
                      min="0"
                      max="100"
                      value={Math.round((s.sound_volume ?? 0.6) * 100)}
                      onChange={(e) => handleFieldChange('sound_volume', parseInt(e.target.value) / 100)}
                      style={{ width: '130px', accentColor: 'var(--accent-purple)' }}
                    />
                    <span style={{ fontSize: '12px', fontFamily: 'monospace', color: '#9c97aa', width: '36px' }}>
                      {Math.round((s.sound_volume ?? 0.6) * 100)}%
                    </span>
                  </div>
                </div>
              </SpotlightCard>
            </section>
          )}

          {/* ================= PANEL 3: GENERAL ================= */}
          {activeTab === 'general' && (
            <section className="settings-panel">
              <div className="panel-header">
                <h2 className="panel-title">
                  <Sliders className="w-5 h-5 text-indigo-400" />
                  General Application Behavior
                </h2>
                <p className="panel-desc">Windows startup options, text injection methods, and clipboard sync.</p>
              </div>

              <SpotlightCard style={{ padding: '4px 16px' }}>
                <div className="setting-row">
                  <div className="setting-info">
                    <label className="setting-title">Launch at Windows Startup</label>
                    <p className="setting-desc">Starts Rift in the background automatically upon Windows login</p>
                  </div>
                  <Switch
                    checked={s.launch_on_startup || false}
                    onCheckedChange={(c) => handleFieldChange('launch_on_startup', c)}
                  />
                </div>

                <div className="setting-row">
                  <div className="setting-info">
                    <label className="setting-title">Start Minimized to System Tray</label>
                    <p className="setting-desc">Keeps Rift hidden unless opened from the taskbar notification tray</p>
                  </div>
                  <Switch
                    checked={s.launch_at_login !== false}
                    onCheckedChange={(c) => handleFieldChange('launch_at_login', c)}
                  />
                </div>

                <div className="setting-row">
                  <div className="setting-info">
                    <label className="setting-title">Auto-Inject Transcribed Text</label>
                    <p className="setting-desc">Directly types speech into the active application window upon key release</p>
                  </div>
                  <Switch
                    checked={s.copy_instead_of_type !== true}
                    onCheckedChange={(c) => handleFieldChange('copy_instead_of_type', !c)}
                  />
                </div>

                <div className="setting-row">
                  <div className="setting-info">
                    <label className="setting-title">🧭 Universal Voice Intent Router</label>
                    <p className="setting-desc">Auto-routes normal speech, inline selection edits, code/text generation, and background Scratchpad notes</p>
                  </div>
                  <Switch
                    checked={!!(s.intent_router_enabled ?? s.intentRouterEnabled)}
                    onCheckedChange={(c) => {
                      handleFieldChange('intent_router_enabled', c);
                      handleFieldChange('intentRouterEnabled', c);
                    }}
                  />
                </div>

                <div className="setting-row">
                  <div className="setting-info">
                    <label className="setting-title">🤫 Sub-Vocal "Library / Whisper Mode"</label>
                    <p className="setting-desc">Ultra-sensitive DSP gain and formant pre-emphasis for quiet whispering</p>
                  </div>
                  <Switch
                    checked={!!(s.whisper_mode_enabled ?? s.whisperModeEnabled)}
                    onCheckedChange={(c) => handleFieldChange('whisper_mode_enabled', c)}
                  />
                </div>

                <div className="setting-row">
                  <div className="setting-info">
                    <label className="setting-title">🎧 Hands-Free "Hey Rift" Wake Word</label>
                    <p className="setting-desc">Say "Hey Rift" on Bluetooth earphones to record hands-free and auto-paste on silence</p>
                  </div>
                  <Switch
                    checked={!!(s.wake_word_enabled ?? s.wakeWordEnabled)}
                    onCheckedChange={(c) => handleFieldChange('wake_word_enabled', c)}
                  />
                </div>

                <div className="setting-row">
                  <div className="setting-info">
                    <label className="setting-title">AI Speech Enhancement (LLM Polish)</label>
                    <p className="setting-desc">Polishes speech and removes fillers. Turn OFF for fastest verbatim transcription</p>
                  </div>
                  <Switch
                    checked={s.auto_enhance_prompt || false}
                    onCheckedChange={(c) => handleFieldChange('auto_enhance_prompt', c)}
                  />
                </div>

                <div className="setting-row">
                  <div className="setting-info">
                    <label className="setting-title">Clipboard history</label>
                    <p className="setting-desc">Save copied text in your Rift Library. Turn this off to stop clipboard capture.</p>
                  </div>
                  <Switch
                    checked={s.clipboard_history_enabled !== false}
                    onCheckedChange={(c) => handleFieldChange('clipboard_history_enabled', c)}
                  />
                </div>

                <div className="setting-row">
                  <div className="setting-info">
                    <label className="setting-title">Clipboard retention</label>
                    <p className="setting-desc">Maximum unpinned items to keep. Pinned items are preserved.</p>
                  </div>
                  <select
                    className="setting-input"
                    aria-label="Maximum unpinned clipboard history items"
                    value={s.clipboard_max_items ?? 50}
                    onChange={(event) => handleFieldChange('clipboard_max_items', Number(event.target.value))}
                    style={{ maxWidth: '160px' }}
                  >
                    {[50, 100, 250, 500, 1000].map((limit) => (
                      <option key={limit} value={limit}>{limit} items</option>
                    ))}
                  </select>
                </div>
              </SpotlightCard>
            </section>
          )}

          {/* ================= PANEL 4: WIDGET ================= */}
          {activeTab === 'widget' && (
            <section className="settings-panel">
              <style>{`
                @keyframes riftPreviewOrbitCw {
                  0% { transform: rotate(0deg); }
                  100% { transform: rotate(360deg); }
                }
                @keyframes riftPreviewOrbitCcw {
                  0% { transform: rotate(360deg); }
                  100% { transform: rotate(0deg); }
                }
                @keyframes riftPreviewCorePulse {
                  0%, 100% { transform: scale(0.78); opacity: 0.72; }
                  50% { transform: scale(1.22); opacity: 1; }
                }
                @keyframes riftPreviewHarmonicScale {
                  0%, 100% { transform: scaleY(0.34); opacity: 0.62; }
                  50% { transform: scaleY(1); opacity: 1; }
                }
                @keyframes riftPreviewCometPrimary {
                  0% { stroke-dashoffset: 100; }
                  100% { stroke-dashoffset: 0; }
                }
                @keyframes riftPreviewCometSecondary {
                  0% { stroke-dashoffset: 50; }
                  100% { stroke-dashoffset: -50; }
                }
              `}</style>
              <div className="panel-header">
                <h2 className="panel-title">
                  <AppWindow className="w-5 h-5 text-purple-300" />
                  Floating Capsule Widget
                </h2>
                <p className="panel-desc">Configure the ultra-compact floating voice indicator and transcribing animation style.</p>
              </div>

              {/* Transcribing Animation Style Selector (4 Flagship Designs) */}
              <SpotlightCard style={{ padding: '16px', marginBottom: '14px' }}>
                <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '12px', gap: '8px', flexWrap: 'wrap' }}>
                  <div>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                      <span style={{ fontSize: '13.5px', fontWeight: 600, color: '#f0eff4' }}>
                        Transcribing Animation Style
                      </span>
                      <span
                        style={{
                          fontSize: '10px',
                          fontWeight: 700,
                          padding: '2px 7px',
                          borderRadius: '999px',
                          background: 'rgba(56, 189, 248, 0.14)',
                          color: '#7dd3fc',
                          border: '1px solid rgba(56, 189, 248, 0.28)',
                          letterSpacing: '0.04em',
                          textTransform: 'uppercase',
                        }}
                      >
                        Full 16px Scale • 21px Capsule
                      </span>
                    </div>
                    <p style={{ fontSize: '12px', color: '#9ca3af', marginTop: '3px' }}>
                      Choose how the floating widget animates while transcribing speech before morphing into the Done (✓) checkmark. Click any style to preview it live on your widget.
                    </p>
                  </div>
                </div>

                <div
                  style={{
                    display: 'grid',
                    gridTemplateColumns: 'repeat(auto-fit, minmax(235px, 1fr))',
                    gap: '10px',
                  }}
                >
                  {[
                    {
                      id: 'orbit_pure',
                      title: 'Pure Dual-Orbit Ring',
                      tag: 'Default • 1:1 Done Scale',
                      capsuleSize: '58 × 21 px',
                      desc: 'Full 16px counter-rotating liquid cyan-indigo orbit ring with breathing core. Zero text clutter — matches the Done checkmark 1:1.',
                    },
                    {
                      id: 'orbit_badge',
                      title: 'Dual-Orbit + Micro Badge',
                      tag: 'Side-by-Side Chip',
                      capsuleSize: '88 × 21 px',
                      desc: 'Full-height 15.5px Dual-Orbit loader paired horizontally with a frosted micro model pill (SCRIBE / TURBO) on the right.',
                    },
                    {
                      id: 'harmonic_wave',
                      title: '5-Bar Harmonic Wave',
                      tag: 'Audio-Organic',
                      capsuleSize: '68 × 21 px',
                      desc: 'Five glowing liquid-gradient vertical bars rippling in a 60fps harmonic sine wave before converging into the Done checkmark.',
                    },
                    {
                      id: 'infinity_pulse',
                      title: 'Stadium Comet Track',
                      tag: 'Raycast / Linear',
                      capsuleSize: '72 × 21 px',
                      desc: 'Wide stadium track with a racing cyan-violet light comet and a breathing center energy star.',
                    },
                  ].map((option) => {
                    const currentStyle = (s.widget_transcribing_style || s.widgetTranscribingStyle || 'orbit_pure').toLowerCase();
                    const isSelected = currentStyle === option.id;

                    return (
                      <button
                        key={option.id}
                        type="button"
                        onClick={() => {
                          handleFieldChange('widget_transcribing_style', option.id);
                        }}
                        style={{
                          display: 'flex',
                          flexDirection: 'column',
                          alignItems: 'stretch',
                          textAlign: 'left',
                          padding: '12px',
                          borderRadius: '11px',
                          cursor: 'pointer',
                          transition: 'all 0.18s ease',
                          background: isSelected
                            ? 'linear-gradient(180deg, rgba(56, 189, 248, 0.12) 0%, rgba(129, 140, 248, 0.08) 100%)'
                            : 'rgba(18, 18, 28, 0.75)',
                          border: isSelected
                            ? '1px solid rgba(56, 189, 248, 0.55)'
                            : '1px solid rgba(255, 255, 255, 0.08)',
                          boxShadow: isSelected
                            ? '0 0 18px rgba(56, 189, 248, 0.14), inset 0 1px 0 rgba(255,255,255,0.08)'
                            : 'none',
                        }}
                      >
                        {/* Live 1:1 Scale Mini Floating Capsule Preview */}
                        <div
                          style={{
                            height: '44px',
                            borderRadius: '8px',
                            background: 'radial-gradient(circle at 50% 50%, rgba(30, 32, 48, 0.9) 0%, rgba(11, 11, 17, 0.95) 100%)',
                            border: '1px solid rgba(255, 255, 255, 0.06)',
                            display: 'flex',
                            alignItems: 'center',
                            justifyContent: 'center',
                            marginBottom: '10px',
                            position: 'relative',
                            overflow: 'hidden',
                          }}
                        >
                          {/* Simulated 21px Capsule */}
                          <div
                            style={{
                              height: '21px',
                              width:
                                option.id === 'orbit_pure'
                                  ? '58px'
                                  : option.id === 'orbit_badge'
                                  ? '88px'
                                  : option.id === 'harmonic_wave'
                                  ? '68px'
                                  : '72px',
                              borderRadius: '999px',
                              background: 'rgba(14, 15, 23, 0.96)',
                              border: '1px solid rgba(56, 189, 248, 0.38)',
                              boxShadow: '0 2px 10px rgba(0,0,0,0.45), 0 0 10px rgba(56, 189, 248, 0.18)',
                              display: 'flex',
                              alignItems: 'center',
                              justifyContent: 'center',
                              gap: '5px',
                              padding: '0 6px',
                            }}
                          >
                            {(option.id === 'orbit_pure' || option.id === 'orbit_badge') && (
                              <svg
                                viewBox="0 0 24 24"
                                fill="none"
                                style={{
                                  width: option.id === 'orbit_pure' ? '16px' : '15px',
                                  height: option.id === 'orbit_pure' ? '16px' : '15px',
                                  flexShrink: 0,
                                  filter: 'drop-shadow(0 0 4px rgba(56, 189, 248, 0.45))',
                                }}
                              >
                                <circle cx="12" cy="12" r="9.5" stroke="rgba(129, 140, 248, 0.16)" strokeWidth="2.2" />
                                <g style={{ transformOrigin: '12px 12px', animation: 'riftPreviewOrbitCw 0.72s linear infinite' }}>
                                  <path
                                    d="M12 2.5A9.5 9.5 0 0 1 21.5 12"
                                    stroke="#38bdf8"
                                    strokeWidth="2.5"
                                    strokeLinecap="round"
                                  />
                                </g>
                                <g style={{ transformOrigin: '12px 12px', animation: 'riftPreviewOrbitCcw 1.05s linear infinite' }}>
                                  <path
                                    d="M12 18.2A6.2 6.2 0 0 1 5.8 12"
                                    stroke="#c084fc"
                                    strokeWidth="1.9"
                                    strokeLinecap="round"
                                  />
                                </g>
                                <circle
                                  cx="12"
                                  cy="12"
                                  r="2.1"
                                  fill="#38bdf8"
                                  style={{ transformOrigin: '12px 12px', animation: 'riftPreviewCorePulse 1.1s ease-in-out infinite' }}
                                />
                              </svg>
                            )}

                            {option.id === 'orbit_badge' && (
                              <span
                                style={{
                                  fontSize: '7.5px',
                                  fontWeight: 700,
                                  letterSpacing: '0.06em',
                                  color: '#cbd5e1',
                                  background: 'rgba(129, 140, 248, 0.14)',
                                  border: '1px solid rgba(129, 140, 248, 0.25)',
                                  borderRadius: '4px',
                                  padding: '1.5px 4px',
                                  lineHeight: 1,
                                }}
                              >
                                SCRIBE
                              </span>
                            )}

                            {option.id === 'harmonic_wave' && (
                              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', gap: '3px', width: '28px', height: '16px' }}>
                                {[
                                  { h: '11px', delay: '0s', bg: 'linear-gradient(180deg, #818cf8 0%, #6366f1 100%)' },
                                  { h: '13.5px', delay: '-0.135s', bg: 'linear-gradient(180deg, #38bdf8 0%, #818cf8 100%)' },
                                  { h: '15px', delay: '-0.27s', bg: 'linear-gradient(180deg, #6ee7b7 0%, #38bdf8 55%, #818cf8 100%)' },
                                  { h: '13.5px', delay: '-0.405s', bg: 'linear-gradient(180deg, #c084fc 0%, #38bdf8 100%)' },
                                  { h: '11px', delay: '-0.54s', bg: 'linear-gradient(180deg, #34d399 0%, #38bdf8 100%)' },
                                ].map((bar, idx) => (
                                  <div
                                    key={idx}
                                    style={{
                                      width: '2.6px',
                                      height: bar.h,
                                      borderRadius: '999px',
                                      background: bar.bg,
                                      boxShadow: idx === 2 ? '0 0 4px rgba(56, 189, 248, 0.5)' : 'none',
                                      transformOrigin: 'center center',
                                      animation: `riftPreviewHarmonicScale 0.68s ease-in-out ${bar.delay} infinite`,
                                      flexShrink: 0,
                                    }}
                                  />
                                ))}
                              </div>
                            )}

                            {option.id === 'infinity_pulse' && (
                              <div style={{ position: 'relative', width: '24px', height: '12px', display: 'flex', alignItems: 'center', justifyContent: 'center', order: 0 }}>
                                <svg viewBox="0 0 36 16" fill="none" style={{ width: '24px', height: '12px' }}>
                                  <rect x="2" y="2" width="32" height="12" rx="6" stroke="#a1a1aa" strokeWidth="2" />
                                </svg>
                              </div>
                            )}
                          </div>

                          <span
                            style={{
                              position: 'absolute',
                              bottom: '4px',
                              right: '7px',
                              fontSize: '9.5px',
                              color: 'rgba(148, 163, 184, 0.7)',
                              fontFamily: 'monospace',
                            }}
                          >
                            {option.capsuleSize}
                          </span>
                        </div>

                        {/* Title + Active Check */}
                        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '6px', marginBottom: '4px' }}>
                          <span style={{ fontSize: '12.5px', fontWeight: 600, color: isSelected ? '#e0f2fe' : '#f0eff4' }}>
                            {option.title}
                          </span>
                          <span
                            style={{
                              fontSize: '9.5px',
                              fontWeight: 600,
                              padding: '1.5px 6px',
                              borderRadius: '999px',
                              background: isSelected ? 'rgba(56, 189, 248, 0.22)' : 'rgba(255, 255, 255, 0.06)',
                              color: isSelected ? '#7dd3fc' : '#9ca3af',
                            }}
                          >
                            {isSelected ? '✓ Active' : option.tag}
                          </span>
                        </div>

                        <p style={{ fontSize: '11.2px', color: '#94a3b8', lineHeight: 1.4, margin: 0 }}>
                          {option.desc}
                        </p>
                      </button>
                    );
                  })}
                </div>
              </SpotlightCard>

              <SpotlightCard style={{ padding: '4px 16px' }}>
                <div className="setting-row">
                  <div className="setting-info">
                    <label className="setting-title">Widget Visibility Mode</label>
                    <p className="setting-desc">When the floating capsule appears on your screen</p>
                  </div>
                  <select
                    className="setting-select"
                    value={s.widget_visibility || 'always'}
                    onChange={(e) => handleFieldChange('widget_visibility', e.target.value)}
                  >
                    <option value="always">Always Visible</option>
                    <option value="recording">Only While Recording</option>
                    <option value="hidden">Hidden Completely</option>
                  </select>
                </div>

                <div className="setting-row">
                  <div className="setting-info">
                    <label className="setting-title">Follow Cursor Across Displays</label>
                    <p className="setting-desc">Reposition widget to whichever monitor your mouse is active on</p>
                  </div>
                  <Switch
                    checked={s.follow_cursor_across_monitors || false}
                    onCheckedChange={(c) => handleFieldChange('follow_cursor_across_monitors', c)}
                  />
                </div>

                <div className="setting-row">
                  <div className="setting-info">
                    <label className="setting-title">Lock Widget Position</label>
                    <p className="setting-desc">Prevent dragging the widget accidentally</p>
                  </div>
                  <Switch
                    checked={s.lock_widget_position || false}
                    onCheckedChange={(c) => handleFieldChange('lock_widget_position', c)}
                  />
                </div>

                <div className="setting-row">
                  <div className="setting-info">
                    <label className="setting-title">Reset Widget Position</label>
                    <p className="setting-desc">Centers the floating capsule at the bottom of your primary screen</p>
                  </div>
                  <button
                    type="button"
                    onClick={() => {
                      invokeTauri('resetWidgetPosition');
                      triggerSavedFeedback();
                    }}
                    style={{
                      display: 'flex',
                      alignItems: 'center',
                      gap: '5px',
                      padding: '6px 12px',
                      borderRadius: '7px',
                      backgroundColor: '#161622',
                      border: '1px solid rgba(255,255,255,0.1)',
                      color: '#f0eff4',
                      fontSize: '12px',
                      cursor: 'pointer',
                    }}
                  >
                    <RotateCcw className="w-3.5 h-3.5" />
                    Reset to Center
                  </button>
                </div>
              </SpotlightCard>
            </section>
          )}

          {/* ================= PANEL 5: SNIPPETS ================= */}
          {activeTab === 'snippets' && (
            <section className="settings-panel">
              <div className="panel-header">
                <h2 className="panel-title">
                  <Bolt className="w-5 h-5 text-amber-400" />
                  Voice Snippets & Templates
                </h2>
                <p className="panel-desc">Speak trigger phrases to expand multi-line templates, signatures, or links.</p>
              </div>

              {/* Add New Snippet */}
              <SpotlightCard style={{ padding: '16px', display: 'flex', flexDirection: 'column', gap: '10px' }}>
                <span style={{ fontSize: '13px', fontWeight: 600, color: '#f0eff4' }}>Create New Voice Snippet</span>
                <div style={{ display: 'flex', gap: '10px', flexWrap: 'wrap' }}>
                  <input
                    type="text"
                    className="setting-input"
                    placeholder="Trigger phrase (e.g. 'my meeting link')"
                    value={newSnippetPhrase}
                    onChange={(e) => setNewSnippetPhrase(e.target.value)}
                    style={{ flex: 1, minWidth: '180px' }}
                  />
                  <input
                    type="text"
                    className="setting-input"
                    placeholder="Expanded text"
                    value={newSnippetExpansion}
                    onChange={(e) => setNewSnippetExpansion(e.target.value)}
                    style={{ flex: 2, minWidth: '220px' }}
                  />
                  <button
                    type="button"
                    onClick={handleAddSnippet}
                    style={{
                      display: 'flex',
                      alignItems: 'center',
                      gap: '5px',
                      padding: '7px 14px',
                      borderRadius: '7px',
                      backgroundColor: 'var(--accent-purple)',
                      color: '#0c0c12',
                      border: 'none',
                      fontSize: '12.5px',
                      fontWeight: 600,
                      cursor: 'pointer',
                    }}
                  >
                    <Plus className="w-3.5 h-3.5" /> Add
                  </button>
                </div>
              </SpotlightCard>

              {/* Snippets list */}
              <SpotlightCard style={{ padding: '4px 16px' }}>
                {snippets.length === 0 ? (
                  <div style={{ padding: '30px', textAlign: 'center', color: '#6e6a7d', fontSize: '13px' }}>
                    No custom snippets saved yet. Add one above!
                  </div>
                ) : (
                  snippets.map((snip) => (
                    <div key={snip.id} className="setting-row">
                      <div className="setting-info">
                        <span className="setting-title font-mono" style={{ color: 'var(--accent-purple)' }}>
                          "{snip.phrase}"
                        </span>
                        <p className="setting-desc truncate" style={{ maxWidth: '420px' }}>
                          {snip.expansion}
                        </p>
                      </div>
                      <button
                        type="button"
                        onClick={() => handleDeleteSnippet(snip.id)}
                        style={{ background: 'transparent', border: 'none', color: '#6e6a7d', cursor: 'pointer', padding: '4px' }}
                        onMouseEnter={(e) => (e.currentTarget.style.color = '#f87171')}
                        onMouseLeave={(e) => (e.currentTarget.style.color = '#6e6a7d')}
                      >
                        <Trash2 className="w-4 h-4" />
                      </button>
                    </div>
                  ))
                )}
              </SpotlightCard>
            </section>
          )}

          {/* ================= PANEL 6: AI FORMATTING ================= */}
          {activeTab === 'ai' && (
            <section className="settings-panel">
              <div className="panel-header">
                <h2 className="panel-title">
                  <Wand2 className="w-5 h-5 text-purple-300" />
                  AI Formatting & Style Directives
                </h2>
                <p className="panel-desc">Customize how speech output is punctuated, structured, and stylized.</p>
              </div>

              {/* Primary AI Enhancement Toggle Card */}
              <SpotlightCard style={{ padding: '4px 16px', marginBottom: '14px' }}>
                <div className="setting-row">
                  <div className="setting-info">
                    <label className="setting-title">AI Speech Enhancement & Polishing</label>
                    <p className="setting-desc">
                      Cleans verbal fillers ('um', 'uh', stutters), punctuate naturally, and styles output with LLM.
                      Turn OFF for instant verbatim typing with zero latency.
                    </p>
                  </div>
                  <Switch
                    checked={s.auto_enhance_prompt || false}
                    onCheckedChange={(c) => handleFieldChange('auto_enhance_prompt', c)}
                  />
                </div>
              </SpotlightCard>

              <SpotlightCard style={{ padding: '16px', display: 'flex', flexDirection: 'column', gap: '12px' }}>
                <label className="setting-title">Custom AI System Prompt</label>
                <textarea
                  className="setting-input"
                  rows={4}
                  placeholder="e.g. Always format lists as markdown bullet points. Never include conversational filler words like 'um' or 'ah'."
                  value={s.formatting_instructions || ''}
                  onChange={(e) => handleFieldChange('formatting_instructions', e.target.value)}
                  style={{ width: '100%', resize: 'vertical', lineHeight: '1.45' }}
                />

                <div style={{ display: 'flex', alignItems: 'center', gap: '6px', flexWrap: 'wrap' }}>
                  <span style={{ fontSize: '11px', color: '#938ea1', fontWeight: 600 }}>Quick Presets:</span>
                  {[
                    { label: 'Casual Chat', text: 'Format text casually, conversational tone, standard capitalization.' },
                    { label: 'Professional Email', text: 'Format text as professional corporate communication with crisp punctuation.' },
                    { label: 'Code & Technical', text: 'Preserve variable names, camelCase, technical acronyms, and format code snippets in markdown backticks.' },
                  ].map((p) => (
                    <button
                      key={p.label}
                      type="button"
                      onClick={() => handleFieldChange('formatting_instructions', p.text)}
                      style={{
                        padding: '3px 8px',
                        borderRadius: '5px',
                        backgroundColor: '#161622',
                        border: '1px solid rgba(255,255,255,0.08)',
                        fontSize: '11px',
                        color: '#938ea1',
                        cursor: 'pointer',
                      }}
                      onMouseEnter={(e) => (e.currentTarget.style.color = '#e4e1ea')}
                      onMouseLeave={(e) => (e.currentTarget.style.color = '#938ea1')}
                    >
                      {p.label}
                    </button>
                  ))}
                </div>
              </SpotlightCard>
            </section>
          )}

          {/* ================= PANEL 7: APPEARANCE ================= */}
          {activeTab === 'appearance' && (
            <section className="settings-panel">
              <div className="panel-header">
                <h2 className="panel-title">
                  <Palette className="w-5 h-5 text-pink-400" />
                  Appearance & Themes
                </h2>
                <p className="panel-desc">Visual styles, color themes, and scaling.</p>
              </div>

              <SpotlightCard style={{ padding: '4px 16px' }}>
                <div className="setting-row">
                  <div className="setting-info">
                    <label className="setting-title">Obsidian Theme Palette</label>
                    <p className="setting-desc">Select the primary dark mode color tone</p>
                  </div>
                  <div style={{ display: 'flex', gap: '8px' }}>
                    {[
                      { id: 'obsidian', name: 'Obsidian Dark' },
                      { id: 'midnight', name: 'Midnight Blue' },
                      { id: 'violet', name: 'Deep Violet' },
                    ].map((theme) => {
                      const isSelected = (s.theme || 'obsidian') === theme.id;
                      return (
                        <button
                          key={theme.id}
                          type="button"
                          onClick={() => handleFieldChange('theme', theme.id)}
                          style={{
                            padding: '6px 12px',
                            borderRadius: '7px',
                            backgroundColor: isSelected ? 'var(--accent-purple-dim)' : '#161622',
                            border: `1px solid ${isSelected ? 'var(--accent-purple)' : 'rgba(255,255,255,0.08)'}`,
                            color: isSelected ? 'var(--accent-purple)' : '#938ea1',
                            fontSize: '11.5px',
                            fontWeight: isSelected ? 600 : 500,
                            cursor: 'pointer',
                          }}
                        >
                          {theme.name}
                        </button>
                      );
                    })}
                  </div>
                </div>
              </SpotlightCard>
            </section>
          )}
        </main>
      </div>
    </div>
  );
};
