import React, { useEffect, useState, useCallback, useRef } from 'react';
import { useSettings } from '../../context/SettingsContext';
import { invokeTauri, listenTauriEvent } from '../../lib/tauriBridge';
import { SpotlightCard } from '../ui/SpotlightCard';
import { NumberTicker } from '../ui/NumberTicker';
import { AudioOrb } from '../ui/AudioOrb';
import { Badge } from '../ui/Badge';
import { Kbd } from '../ui/Kbd';
import { SimpleTooltip } from '../ui/Tooltip';
import { Sparkles, Mic, Clock, FileText, Zap, ChevronRight, Copy, Check, ArrowUpRight } from 'lucide-react';

interface HistoryEntry {
  id: number | string;
  timestamp: string;
  text: string;
  duration_seconds: number;
  char_count?: number;
  words_count: number;
  target_app?: string;
  model?: string;
  engine?: string;
}

interface RemainingUsage {
  formattedTime?: string;
  subtext?: string;
  dotColor?: string;
  provider?: string;
  balance?: number;
  model?: string;
  remainingSeconds?: number;
  quotaType?: string;
}

interface DashboardStatsData {
  totalWords: number;
  totalSpeakingSeconds: number;
  totalRecordings: number;
  avgPaceWpm: number;
  todayRequests?: number;
  dailyLimit?: number;
  currentModel?: string;
  transcriptionMode?: string;
  sttProvider?: string;
  localInferenceProfile?: string;
  remainingUsage?: RemainingUsage;
}

interface EngineDetails {
  providerName: string;
  badgeText: string;
  badgeVariant: 'groq' | 'openrouter' | 'violet' | 'default' | 'success';
  modelId: string;
  latencyText: string;
  latencyColor: string;
  spotlightColor: string;
}

function resolveEngineDetails(
  modelStr?: string,
  providerStr?: string,
  modeStr?: string,
  localProfile?: string,
  remainingUsage?: RemainingUsage,
  tenglishEnabled?: boolean
): EngineDetails {
  const mode = (modeStr || '').toLowerCase();
  const rawModel = (modelStr || remainingUsage?.model || '').trim();
  const m = rawModel.toLowerCase();
  const prov = (remainingUsage?.provider || providerStr || '').toLowerCase();

  if (mode === 'local' || m.startsWith('local')) {
    const prof = localProfile || 'tiny';
    return {
      providerName: 'Local Offline',
      badgeText: '🔒 Local Offline',
      badgeVariant: 'violet',
      modelId: `whisper-${prof}`,
      latencyText: '0ms Network / Local',
      latencyColor: '#c084fc',
      spotlightColor: 'rgba(168, 85, 247, 0.12)',
    };
  }

  if (tenglishEnabled) {
    return {
      providerName: 'OpenRouter Cloud',
      badgeText: '🇮🇳 Tenglish (Telugu → EN)',
      badgeVariant: 'openrouter',
      modelId: 'microsoft/mai-transcribe-2',
      latencyText: 'Studio Grade (~1.8s)',
      latencyColor: '#818cf8',
      spotlightColor: 'rgba(99, 102, 241, 0.14)',
    };
  }

  // OpenRouter models
  if (
    prov === 'openrouter' ||
    m.includes('mai-transcribe') ||
    m.startsWith('microsoft/') ||
    m.startsWith('openai/')
  ) {
    if (m.includes('mai-transcribe')) {
      return {
        providerName: 'OpenRouter Cloud',
        badgeText: '🏆 OpenRouter Cloud',
        badgeVariant: 'openrouter',
        modelId: 'microsoft/mai-transcribe-2',
        latencyText: 'Studio Grade (~1.8s)',
        latencyColor: '#818cf8',
        spotlightColor: 'rgba(99, 102, 241, 0.14)',
      };
    }
    if (m.includes('turbo')) {
      return {
        providerName: 'OpenRouter Cloud',
        badgeText: '🏆 OpenRouter Cloud',
        badgeVariant: 'openrouter',
        modelId: 'openai/whisper-large-v3-turbo',
        latencyText: 'Fast (~1.8s)',
        latencyColor: '#818cf8',
        spotlightColor: 'rgba(99, 102, 241, 0.14)',
      };
    }
    return {
      providerName: 'OpenRouter Cloud',
      badgeText: '🏆 OpenRouter Cloud',
      badgeVariant: 'openrouter',
      modelId: rawModel || 'openai/whisper-large-v3',
      latencyText: 'Standard (~2.2s)',
      latencyColor: '#818cf8',
      spotlightColor: 'rgba(99, 102, 241, 0.14)',
    };
  }

  // Groq Cloud models
  if (m.includes('whisper-large-v3') && !m.includes('turbo')) {
    return {
      providerName: 'Groq Cloud LPU',
      badgeText: '⚡ Groq Cloud LPU',
      badgeVariant: 'groq',
      modelId: 'whisper-large-v3',
      latencyText: 'Standard (~150ms)',
      latencyColor: '#45dfa9',
      spotlightColor: 'rgba(56, 189, 248, 0.10)',
    };
  }

  // Default Groq Turbo
  const cleanId = rawModel.replace(/^groq\//, '') || 'whisper-large-v3-turbo';
  return {
    providerName: 'Groq Cloud LPU',
    badgeText: '⚡ Groq Cloud LPU',
    badgeVariant: 'groq',
    modelId: cleanId,
    latencyText: '~85ms Latency',
    latencyColor: '#45dfa9',
    spotlightColor: 'rgba(56, 189, 248, 0.10)',
  };
}

interface DashboardScreenProps {
  onNavigateToHistory: () => void;
}

export const DashboardScreen: React.FC<DashboardScreenProps> = ({ onNavigateToHistory }) => {
  const { settings } = useSettings();
  const [stats, setStats] = useState<DashboardStatsData>({
    totalWords: 0,
    totalSpeakingSeconds: 0,
    totalRecordings: 0,
    avgPaceWpm: 0,
    remainingUsage: {
      formattedTime: '--',
      subtext: 'Calculating...',
      dotColor: '#38bdf8',
    },
  });
  const [recentList, setRecentList] = useState<HistoryEntry[]>([]);
  const [copiedId, setCopiedId] = useState<string | number | null>(null);

  // Live Mic Test State for 3D Audio Orb
  const [isMicTesting, setIsMicTesting] = useState(false);
  const [liveAudioLevel, setLiveAudioLevel] = useState(0);
  const audioContextRef = useRef<AudioContext | null>(null);
  const analyserRef = useRef<AnalyserNode | null>(null);
  const animFrameRef = useRef<number | null>(null);

  function formatSpeakingTime(seconds: number) {
    if (!seconds || seconds <= 0) return '0s';
    const totalSec = Math.round(seconds);
    if (totalSec < 60) return `${totalSec}s`;
    const hours = Math.floor(totalSec / 3600);
    const minutes = Math.floor((totalSec % 3600) / 60);
    const remainingSec = totalSec % 60;
    if (hours > 0) return `${hours}h ${minutes}m`;
    return remainingSec > 0 ? `${minutes}m ${remainingSec}s` : `${minutes}m`;
  }

  function formatClockTime(isoString: string) {
    if (!isoString) return 'Recent';
    try {
      const d = new Date(isoString);
      return d.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit', hour12: true });
    } catch {
      return 'Recent';
    }
  }

  const refreshAll = useCallback(async () => {
    try {
      const statsRes = await invokeTauri<DashboardStatsData>('getDashboardStats');
      if (statsRes) {
        setStats(statsRes);
      }
      const histRes = await invokeTauri<HistoryEntry[]>('getHistory', { filter: { limit: 8 } });
      if (Array.isArray(histRes)) {
        setRecentList(histRes);
      }
    } catch (err) {
      console.warn('[Dashboard] Refresh error:', err);
    }
  }, []);

  useEffect(() => {
    refreshAll();

    const unlistens = [
      listenTauriEvent('transcriptionComplete', () => refreshAll()),
      listenTauriEvent('transcriptionSuccess', () => refreshAll()),
      listenTauriEvent('textInjected', () => refreshAll()),
      listenTauriEvent('usageUpdated', () => refreshAll()),
      listenTauriEvent('historyCleared', () => refreshAll()),
      listenTauriEvent('settingsChanged', () => refreshAll()),
    ];

    const interval = setInterval(refreshAll, 3000);

    return () => {
      unlistens.forEach((u) => u());
      clearInterval(interval);
    };
  }, [refreshAll, settings?.inference_model, settings?.stt_provider, settings?.transcription_mode, settings?.tenglish_mode_enabled]);

  // Start / Stop Microphone Test for 3D Visualizer
  const toggleMicTest = async () => {
    if (isMicTesting) {
      setIsMicTesting(false);
      if (animFrameRef.current) cancelAnimationFrame(animFrameRef.current);
      if (audioContextRef.current) {
        audioContextRef.current.close();
        audioContextRef.current = null;
      }
      setLiveAudioLevel(0);
      return;
    }

    try {
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true, video: false });
      const audioCtx = new (window.AudioContext || (window as any).webkitAudioContext)();
      const analyser = audioCtx.createAnalyser();
      analyser.fftSize = 64;
      const source = audioCtx.createMediaStreamSource(stream);
      source.connect(analyser);

      audioContextRef.current = audioCtx;
      analyserRef.current = analyser;
      setIsMicTesting(true);

      const dataArray = new Uint8Array(analyser.frequencyBinCount);
      const updateLevel = () => {
        if (!analyserRef.current) return;
        analyserRef.current.getByteFrequencyData(dataArray);
        let sum = 0;
        for (let i = 0; i < dataArray.length; i++) {
          sum += dataArray[i];
        }
        const avg = sum / dataArray.length;
        setLiveAudioLevel(avg / 128); // 0.0 to 1.0 approx
        animFrameRef.current = requestAnimationFrame(updateLevel);
      };
      updateLevel();
    } catch (e) {
      console.warn('Microphone access for visualizer test unavailable:', e);
      setIsMicTesting(false);
    }
  };

  useEffect(() => {
    return () => {
      if (animFrameRef.current) cancelAnimationFrame(animFrameRef.current);
      if (audioContextRef.current) audioContextRef.current.close();
    };
  }, []);

  const handleCopyText = (id: string | number, text: string) => {
    navigator.clipboard.writeText(text);
    setCopiedId(id);
    setTimeout(() => setCopiedId(null), 1800);
  };

  const pttKey = settings?.push_to_talk_key || 'Right Alt';

  const engineDetails = resolveEngineDetails(
    stats.currentModel || settings?.inference_model || (settings as any)?.inferenceModel,
    settings?.stt_provider || (stats as any)?.sttProvider,
    stats.transcriptionMode || settings?.transcription_mode,
    (stats as any)?.localInferenceProfile || settings?.local_inference_profile,
    stats.remainingUsage,
    settings?.tenglish_mode_enabled
  );

  const remainingTimeStr = stats.remainingUsage?.formattedTime || (engineDetails.badgeVariant === 'violet' ? 'Unlimited' : '--');
  const quotaSubtext = stats.remainingUsage?.subtext || (
    engineDetails.badgeVariant === 'openrouter' ? `${engineDetails.modelId.includes('mai-transcribe') ? 'MAI-Transcribe 2' : 'OpenRouter Cloud'} · Ready` :
    engineDetails.badgeVariant === 'violet' ? 'Offline Local Whisper' : 'Groq Cloud Tier'
  );
  const quotaDotColor = stats.remainingUsage?.dotColor || (
    engineDetails.badgeVariant === 'openrouter' ? '#818cf8' :
    engineDetails.badgeVariant === 'violet' ? '#c084fc' : '#45dfa9'
  );

  return (
    <div className="rift-main-area">
      <div style={{ maxWidth: '1040px', margin: '0 auto', display: 'flex', flexDirection: 'column', gap: '22px' }}>
        
        {/* ================= BENTO ROW 1: 3D VOICE HERO & QUOTA GAUGE ================= */}
        <div style={{ display: 'grid', gridTemplateColumns: 'minmax(0, 1.8fr) minmax(0, 1.2fr)', gap: '16px' }}>
          
          {/* Card 1: 3D Interactive Audio Hero Card */}
          <SpotlightCard
            spotlightColor="rgba(168, 85, 247, 0.12)"
            style={{
              padding: '24px',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              gap: '20px',
              minHeight: '200px',
            }}
          >
            <div style={{ display: 'flex', flexDirection: 'column', gap: '12px', zIndex: 2 }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                <Badge variant="violet" size="sm">
                  <Sparkles style={{ width: '12px', height: '12px', color: '#c084fc' }} />
                  Zero Latency Dictation
                </Badge>
              </div>

              <div>
                <h1 style={{ margin: 0, fontSize: '24px', fontWeight: 700, color: '#f0eff4', letterSpacing: '-0.02em' }}>
                  Ready to Dictate
                </h1>
                <p style={{ margin: '4px 0 0', fontSize: '13px', color: '#9c97aa', lineHeight: 1.5 }}>
                  Hold <Kbd size="md" className="mx-1">{pttKey}</Kbd> in any application to type at lightning speed.
                </p>
              </div>

              <div style={{ display: 'flex', alignItems: 'center', gap: '10px', marginTop: '4px' }}>
                <button
                  type="button"
                  onClick={toggleMicTest}
                  style={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: '7px',
                    padding: '7px 14px',
                    borderRadius: '8px',
                    backgroundColor: isMicTesting ? 'rgba(239, 68, 68, 0.15)' : 'rgba(168, 85, 247, 0.15)',
                    border: `1px solid ${isMicTesting ? 'rgba(239, 68, 68, 0.35)' : 'rgba(168, 85, 247, 0.35)'}`,
                    color: isMicTesting ? '#f87171' : '#c084fc',
                    fontSize: '12px',
                    fontWeight: 600,
                    cursor: 'pointer',
                    transition: 'all 0.15s ease',
                  }}
                >
                  <Mic style={{ width: '14px', height: '14px' }} />
                  {isMicTesting ? 'Stop Mic Test' : 'Test Live Audio Wave'}
                </button>

                {isMicTesting && (
                  <span style={{ fontSize: '11px', color: '#45dfa9', display: 'flex', alignItems: 'center', gap: '5px' }}>
                    <span className="live-pulse-dot" style={{ backgroundColor: '#45dfa9' }} />
                    Live Input Active
                  </span>
                )}
              </div>
            </div>

            {/* 3D WebGL Sphere Visualizer */}
            <div style={{ position: 'relative', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
              <AudioOrb
                size={175}
                active={isMicTesting}
                audioLevel={liveAudioLevel}
                color="#a855f7"
                glowColor="#38bdf8"
              />
            </div>
          </SpotlightCard>

          {/* Card 2: Live Engine & Quota Status Card */}
          <SpotlightCard
            spotlightColor={engineDetails.spotlightColor}
            style={{
              padding: '24px',
              display: 'flex',
              flexDirection: 'column',
              justifyContent: 'space-between',
              gap: '16px',
            }}
          >
            <div>
              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                <span style={{ fontSize: '11px', textTransform: 'uppercase', letterSpacing: '0.06em', color: '#9c97aa', fontWeight: 600 }}>
                  Active STT Engine
                </span>
                <Badge variant={engineDetails.badgeVariant}>{engineDetails.badgeText}</Badge>
              </div>

              <div style={{ marginTop: '14px' }}>
                <span style={{ fontSize: '12.5px', color: '#9c97aa' }}>Estimated Speech Usage Left</span>
                <div style={{ display: 'flex', alignItems: 'baseline', gap: '8px', marginTop: '2px' }}>
                  <span style={{ fontSize: '28px', fontWeight: 800, color: '#f0eff4', fontFamily: "'JetBrains Mono', monospace" }}>
                    {remainingTimeStr}
                  </span>
                  <div style={{ display: 'flex', alignItems: 'center', gap: '5px' }}>
                    <span className="live-pulse-dot" style={{ backgroundColor: quotaDotColor }} />
                    <span style={{ fontSize: '12px', color: quotaDotColor, fontWeight: 500 }}>
                      {quotaSubtext}
                    </span>
                  </div>
                </div>
              </div>
            </div>

            <div style={{ borderTop: '1px solid rgba(255, 255, 255, 0.06)', paddingTop: '12px', display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
              <span style={{ fontSize: '11.5px', color: '#6e6a7d' }}>
                Model: <span style={{ color: '#cac4d7', fontFamily: 'monospace' }}>{engineDetails.modelId}</span>
              </span>
              <span style={{ fontSize: '11.5px', color: engineDetails.latencyColor, fontWeight: 600 }}>
                {engineDetails.latencyText}
              </span>
            </div>
          </SpotlightCard>
        </div>

        {/* ================= BENTO ROW 2: 4 LIFETIME STATS (SPOTLIGHT & NUMBER TICKER) ================= */}
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, minmax(0, 1fr))', gap: '14px' }}>
          
          {/* Stat 1: Total Words */}
          <SpotlightCard spotlightColor="rgba(196, 181, 253, 0.12)" style={{ padding: '18px 20px' }}>
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '8px' }}>
              <span style={{ fontSize: '12px', fontWeight: 600, color: '#9c97aa' }}>Words Spoken</span>
              <FileText style={{ width: '16px', height: '16px', color: '#c4b5fd' }} />
            </div>
            <div style={{ fontSize: '26px', fontWeight: 800, color: '#f0eff4', fontFamily: "'JetBrains Mono', monospace" }}>
              <NumberTicker value={stats.totalWords} duration={1200} />
            </div>
            <span style={{ fontSize: '11px', color: '#6e6a7d', marginTop: '4px', display: 'block' }}>
              Lifetime dictation volume
            </span>
          </SpotlightCard>

          {/* Stat 2: Total Speaking Time */}
          <SpotlightCard spotlightColor="rgba(52, 211, 153, 0.12)" style={{ padding: '18px 20px' }}>
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '8px' }}>
              <span style={{ fontSize: '12px', fontWeight: 600, color: '#9c97aa' }}>Speaking Time</span>
              <Clock style={{ width: '16px', height: '16px', color: '#34d399' }} />
            </div>
            <div style={{ fontSize: '26px', fontWeight: 800, color: '#f0eff4', fontFamily: "'JetBrains Mono', monospace" }}>
              {formatSpeakingTime(stats.totalSpeakingSeconds)}
            </div>
            <span style={{ fontSize: '11px', color: '#6e6a7d', marginTop: '4px', display: 'block' }}>
              Total microphone speech
            </span>
          </SpotlightCard>

          {/* Stat 3: Total Recordings */}
          <SpotlightCard spotlightColor="rgba(147, 197, 253, 0.12)" style={{ padding: '18px 20px' }}>
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '8px' }}>
              <span style={{ fontSize: '12px', fontWeight: 600, color: '#9c97aa' }}>Total Sessions</span>
              <Mic style={{ width: '16px', height: '16px', color: '#93c5fd' }} />
            </div>
            <div style={{ fontSize: '26px', fontWeight: 800, color: '#f0eff4', fontFamily: "'JetBrains Mono', monospace" }}>
              <NumberTicker value={stats.totalRecordings} duration={1000} />
            </div>
            <span style={{ fontSize: '11px', color: '#6e6a7d', marginTop: '4px', display: 'block' }}>
              Completed recordings
            </span>
          </SpotlightCard>

          {/* Stat 4: Average Speaking Pace */}
          <SpotlightCard spotlightColor="rgba(250, 204, 21, 0.12)" style={{ padding: '18px 20px' }}>
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '8px' }}>
              <span style={{ fontSize: '12px', fontWeight: 600, color: '#9c97aa' }}>Average Pace</span>
              <Zap style={{ width: '16px', height: '16px', color: '#facc15' }} />
            </div>
            <div style={{ fontSize: '26px', fontWeight: 800, color: '#f0eff4', fontFamily: "'JetBrains Mono', monospace" }}>
              <NumberTicker value={stats.avgPaceWpm} duration={800} /> <span style={{ fontSize: '14px', fontWeight: 600, color: '#9c97aa' }}>WPM</span>
            </div>
            <span style={{ fontSize: '11px', color: '#6e6a7d', marginTop: '4px', display: 'block' }}>
              Words per minute rate
            </span>
          </SpotlightCard>
        </div>

        {/* ================= BENTO ROW 3: RECENT ACTIVITY FEED ================= */}
        <SpotlightCard spotlightColor="rgba(255, 255, 255, 0.04)" style={{ padding: '20px 24px' }}>
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '16px' }}>
            <div>
              <h2 style={{ margin: 0, fontSize: '16px', fontWeight: 600, color: '#f0eff4' }}>
                Recent Transcriptions
              </h2>
              <span style={{ fontSize: '11.5px', color: '#9c97aa' }}>
                Instant playback and clipboard history
              </span>
            </div>

            <button
              type="button"
              onClick={onNavigateToHistory}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: '4px',
                background: 'transparent',
                border: 'none',
                color: '#c4b5fd',
                fontSize: '12.5px',
                fontWeight: 600,
                cursor: 'pointer',
              }}
            >
              View Full History <ChevronRight style={{ width: '15px', height: '15px' }} />
            </button>
          </div>

          {recentList.length === 0 ? (
            <div style={{ padding: '36px', textAlign: 'center', color: '#6e6a7d', fontSize: '13px' }}>
              No dictations recorded yet. Press and hold <Kbd size="sm" className="mx-1">{pttKey}</Kbd> to start your first session.
            </div>
          ) : (
            <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
              {recentList.map((entry) => {
                const isCopied = copiedId === entry.id;
                return (
                  <div
                    key={entry.id}
                    style={{
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'space-between',
                      padding: '12px 16px',
                      borderRadius: '8px',
                      backgroundColor: 'rgba(255, 255, 255, 0.02)',
                      border: '1px solid rgba(255, 255, 255, 0.04)',
                      transition: 'background-color 0.15s, border-color 0.15s',
                    }}
                    onMouseEnter={(e) => {
                      e.currentTarget.style.backgroundColor = 'rgba(255, 255, 255, 0.04)';
                      e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.09)';
                    }}
                    onMouseLeave={(e) => {
                      e.currentTarget.style.backgroundColor = 'rgba(255, 255, 255, 0.02)';
                      e.currentTarget.style.borderColor = 'rgba(255, 255, 255, 0.04)';
                    }}
                  >
                    <div style={{ display: 'flex', flexDirection: 'column', gap: '4px', maxWidth: '82%' }}>
                      <p
                        style={{
                          margin: 0,
                          fontSize: '13px',
                          color: '#f0eff4',
                          lineHeight: 1.45,
                          whiteSpace: 'nowrap',
                          overflow: 'hidden',
                          textOverflow: 'ellipsis',
                        }}
                      >
                        {entry.text}
                      </p>
                      <div style={{ display: 'flex', alignItems: 'center', gap: '8px', fontSize: '11px', color: '#6e6a7d' }}>
                        <span>{formatClockTime(entry.timestamp)}</span>
                        <span>•</span>
                        <span>{entry.words_count || (entry.text ? entry.text.trim().split(/\s+/).filter(Boolean).length : 0)} words</span>
                        <span>•</span>
                        <span>{entry.duration_seconds ? `${entry.duration_seconds.toFixed(1)}s` : '0s'}</span>
                        {entry.model && (
                          <>
                            <span>•</span>
                            <span style={{ color: '#9c97aa', fontFamily: 'monospace' }}>
                              {entry.model.replace(/^groq\//, '').replace(/^openai\//, '').replace(/^microsoft\//, '')}
                            </span>
                          </>
                        )}
                      </div>
                    </div>

                    <SimpleTooltip content={isCopied ? 'Copied!' : 'Copy to Clipboard'}>
                      <button
                        type="button"
                        onClick={() => handleCopyText(entry.id, entry.text)}
                        style={{
                          padding: '6px 10px',
                          borderRadius: '6px',
                          backgroundColor: isCopied ? 'rgba(52, 211, 153, 0.15)' : 'rgba(255, 255, 255, 0.05)',
                          border: `1px solid ${isCopied ? 'rgba(52, 211, 153, 0.3)' : 'rgba(255, 255, 255, 0.08)'}`,
                          color: isCopied ? '#34d399' : '#9c97aa',
                          cursor: 'pointer',
                          display: 'flex',
                          alignItems: 'center',
                          gap: '5px',
                          fontSize: '11.5px',
                          transition: 'all 0.15s ease',
                        }}
                      >
                        {isCopied ? (
                          <>
                            <Check style={{ width: '13px', height: '13px' }} />
                            <span>Copied</span>
                          </>
                        ) : (
                          <>
                            <Copy style={{ width: '13px', height: '13px' }} />
                            <span>Copy</span>
                          </>
                        )}
                      </button>
                    </SimpleTooltip>
                  </div>
                );
              })}
            </div>
          )}
        </SpotlightCard>
      </div>
    </div>
  );
};
