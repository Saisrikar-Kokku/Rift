import React, { useState, useEffect } from 'react';
import { useSettings } from '../../context/SettingsContext';
import { listenTauriEvent } from '../../lib/tauriBridge';
import { Kbd } from '../ui/Kbd';
import { SimpleTooltip } from '../ui/Tooltip';
import { Mic, Zap, Check } from 'lucide-react';

interface HeaderProps {
  onOpenSettings: () => void;
}

export const Header: React.FC<HeaderProps> = ({ onOpenSettings }) => {
  const { settings } = useSettings();
  const [recordingState, setRecordingState] = useState<'idle' | 'recording' | 'transcribing' | 'success'>('idle');
  const [transcribeInfo, setTranscribeInfo] = useState<string>('');

  useEffect(() => {
    const unlistens = [
      listenTauriEvent('recordingStarted', () => {
        setRecordingState('recording');
      }),
      listenTauriEvent('recordingStopped', () => {
        setRecordingState('transcribing');
        setTranscribeInfo('Transcribing speech...');
      }),
      listenTauriEvent<any>('transcriptionProcessing', (payload) => {
        try {
          const data = typeof payload === 'string' ? JSON.parse(payload) : payload;
          if (data?.stage === 'enhancing') {
            setTranscribeInfo(data?.submodel ? `${data.submodel}...` : 'AI Polish...');
          } else if (data?.stage === 'transliterating') {
            setTranscribeInfo('Tenglish Translating...');
          } else {
            setTranscribeInfo('Transcribing speech...');
          }
        } catch (e) {
          setTranscribeInfo('Transcribing...');
        }
        setRecordingState('transcribing');
      }),
      listenTauriEvent('transcriptionSuccess', () => {
        setRecordingState('success');
        setTimeout(() => setRecordingState('idle'), 2200);
      }),
      listenTauriEvent('recordingCancelled', () => {
        setRecordingState('idle');
      }),
      listenTauriEvent('recordingSaved', () => {
        setRecordingState('idle');
      }),
      listenTauriEvent('transcriptionError', () => {
        setRecordingState('idle');
      }),
    ];

    return () => {
      unlistens.forEach((u) => u());
    };
  }, []);

  const modelStr = settings?.inference_model || (settings as any)?.inferenceModel || '';
  const modeStr = settings?.transcription_mode || '';
  const isLocal = modeStr === 'local' || modelStr.startsWith('local');
  const isOpenRouter =
    !isLocal &&
    (modelStr.includes('mai-transcribe') ||
      modelStr.startsWith('microsoft/') ||
      modelStr.startsWith('openai/') ||
      settings?.stt_provider === 'openrouter');

  let engineName = 'Groq Turbo (Free)';
  let engineSub = '• Ultra-Fast';
  let iconColor = 'text-emerald-400';

  if (isLocal) {
    const prof = settings?.local_inference_profile || 'tiny';
    engineName = `Local Whisper (${prof})`;
    engineSub = '• On-Device';
    iconColor = 'text-purple-400';
  } else if (isOpenRouter) {
    if (modelStr.includes('mai-transcribe')) {
      engineName = 'MAI-Transcribe 2';
      engineSub = '• Studio Grade';
      iconColor = 'text-indigo-400';
    } else if (modelStr.includes('turbo')) {
      engineName = 'OpenAI Turbo';
      engineSub = '• Fast Cloud';
      iconColor = 'text-indigo-400';
    } else {
      engineName = 'OpenRouter Cloud';
      engineSub = '• Studio Grade';
      iconColor = 'text-indigo-400';
    }
  } else {
    if (modelStr.includes('whisper-large-v3') && !modelStr.includes('turbo')) {
      engineName = 'Groq Whisper';
      engineSub = '• Standard';
      iconColor = 'text-emerald-400';
    } else {
      engineName = 'Groq Turbo (Free)';
      engineSub = '• Ultra-Fast';
      iconColor = 'text-emerald-400';
    }
  }

  const pttMode = settings?.push_to_talk_mode || 'single';
  const hotkey = (pttMode === 'combo' ? settings?.push_to_talk_combo : settings?.push_to_talk_key) || 'Right Alt';

  return (
    <header className="rift-header">
      {/* Engine Status Badge (Clickable to Settings) */}
      <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
        <SimpleTooltip content="Click to configure speech engine">
          <button
            type="button"
            onClick={onOpenSettings}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: '8px',
              padding: '5px 14px',
              borderRadius: '9999px',
              backgroundColor: '#161622',
              border: '1px solid rgba(255,255,255,0.08)',
              color: '#f0eff4',
              fontSize: '12px',
              fontFamily: "'JetBrains Mono', monospace",
              cursor: 'pointer',
              transition: 'all 0.15s ease',
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.borderColor = 'rgba(167, 139, 250, 0.35)';
              e.currentTarget.style.backgroundColor = '#1d1d2b';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.borderColor = 'rgba(255,255,255,0.08)';
              e.currentTarget.style.backgroundColor = '#161622';
            }}
          >
            <Mic className={`w-3.5 h-3.5 ${iconColor}`} />
            <span style={{ color: '#f0eff4', fontWeight: 600 }}>{engineName}</span>
            <span style={{ color: '#9c97aa' }}>{engineSub}</span>
          </button>
        </SimpleTooltip>
      </div>

      {/* Right Controls: Active Hotkey Keycap & Online / Live Recording Indicator */}
      <div style={{ display: 'flex', alignItems: 'center', gap: '14px' }}>
        <SimpleTooltip content="Active Push-to-Talk Hotkey">
          <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
            <span style={{ fontSize: '11px', color: '#6e6a7d' }}>Trigger:</span>
            <Kbd size="sm">{hotkey}</Kbd>
          </div>
        </SimpleTooltip>

        {/* Live Recording State (Visible in Fullscreen / In-App) */}
        {recordingState === 'recording' && (
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: '8px',
              padding: '4px 12px',
              borderRadius: '9999px',
              backgroundColor: 'rgba(239, 68, 68, 0.15)',
              border: '1px solid rgba(239, 68, 68, 0.4)',
              fontFamily: "'JetBrains Mono', monospace",
              fontSize: '11.5px',
              color: '#f87171',
              boxShadow: '0 0 16px rgba(239, 68, 68, 0.25)',
            }}
          >
            <span style={{ position: 'relative', display: 'flex', height: '8px', width: '8px' }}>
              <span
                className="animate-ping"
                style={{
                  position: 'absolute',
                  display: 'inline-flex',
                  height: '100%',
                  width: '100%',
                  borderRadius: '50%',
                  backgroundColor: '#ef4444',
                  opacity: 0.85,
                }}
              />
              <span
                style={{
                  position: 'relative',
                  display: 'inline-flex',
                  borderRadius: '50%',
                  height: '8px',
                  width: '8px',
                  backgroundColor: '#ef4444',
                }}
              />
            </span>
            <span style={{ fontWeight: 600, color: '#fca5a5' }}>Recording...</span>
          </div>
        )}

        {recordingState === 'transcribing' && (
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: '7px',
              padding: '4px 12px',
              borderRadius: '9999px',
              backgroundColor: 'rgba(167, 139, 250, 0.15)',
              border: '1px solid rgba(167, 139, 250, 0.35)',
              fontFamily: "'JetBrains Mono', monospace",
              fontSize: '11.5px',
              color: '#c4b5fd',
            }}
          >
            <Zap className="w-3.5 h-3.5 text-purple-400" />
            <span style={{ fontWeight: 600 }}>{transcribeInfo || 'Transcribing...'}</span>
          </div>
        )}

        {recordingState === 'success' && (
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: '7px',
              padding: '4px 12px',
              borderRadius: '9999px',
              backgroundColor: 'rgba(52, 211, 153, 0.15)',
              border: '1px solid rgba(52, 211, 153, 0.35)',
              fontFamily: "'JetBrains Mono', monospace",
              fontSize: '11.5px',
              color: '#6ee7b7',
            }}
          >
            <Check className="w-3.5 h-3.5 text-emerald-400" />
            <span style={{ fontWeight: 600 }}>Pasted & Saved</span>
          </div>
        )}

        {recordingState === 'idle' && (
          <div
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: '7px',
              padding: '4px 11px',
              borderRadius: '9999px',
              backgroundColor: '#161622',
              border: '1px solid rgba(255,255,255,0.08)',
              fontFamily: "'JetBrains Mono', monospace",
              fontSize: '11.5px',
              color: '#cac4d7',
            }}
          >
            <span style={{ position: 'relative', display: 'flex', height: '7px', width: '7px' }}>
              <span
                className="animate-ping"
                style={{
                  position: 'absolute',
                  display: 'inline-flex',
                  height: '100%',
                  width: '100%',
                  borderRadius: '50%',
                  backgroundColor: '#34d399',
                  opacity: 0.75,
                }}
              />
              <span
                style={{
                  position: 'relative',
                  display: 'inline-flex',
                  borderRadius: '50%',
                  height: '7px',
                  width: '7px',
                  backgroundColor: '#34d399',
                }}
              />
            </span>
            <span style={{ fontWeight: 500, color: '#34d399' }}>Online</span>
          </div>
        )}
      </div>
    </header>
  );
};
