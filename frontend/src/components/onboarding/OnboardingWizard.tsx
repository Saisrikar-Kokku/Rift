import React, { useState } from 'react';
import { useSettings } from '../../context/SettingsContext';
import { useAuth } from '../../context/AuthContext';
import { invokeTauri } from '../../lib/tauriBridge';
import { AuroraBackground } from '../ui/AuroraBackground';
import { SpotlightCard } from '../ui/SpotlightCard';
import { AudioOrb } from '../ui/AudioOrb';
import { Kbd } from '../ui/Kbd';
import { Badge } from '../ui/Badge';
import confetti from 'canvas-confetti';
import {
  Sparkles,
  KeyRound,
  Keyboard,
  ArrowRight,
  CheckCircle2,
  ExternalLink,
  Mic,
  Check,
  Zap,
} from 'lucide-react';

interface OnboardingWizardProps {
  onComplete: () => void;
}

export const OnboardingWizard: React.FC<OnboardingWizardProps> = ({ onComplete }) => {
  const { settings, updateSettings } = useSettings();
  const { user, updateProfile } = useAuth();

  const [step, setStep] = useState<1 | 2 | 3>(1);
  const [apiKey, setApiKey] = useState('');
  const [keySaving, setKeySaving] = useState(false);
  const [keyVerified, setKeyVerified] = useState(false);
  const [keyError, setKeyError] = useState<string | null>(null);

  // Hotkey selection
  const [selectedKey, setSelectedKey] = useState(settings.push_to_talk_key || 'Right Alt');
  const [soundEnabled, setSoundEnabled] = useState(settings.recording_sounds ?? true);

  const handleSaveApiKey = async () => {
    if (!apiKey.trim()) {
      setKeyError('Please paste your Groq API key.');
      return;
    }
    setKeySaving(true);
    setKeyError(null);
    try {
      const res = await invokeTauri<{ valid?: boolean; success?: boolean; message?: string }>('testGroqKey', {
        key: apiKey.trim(),
      });
      if (res?.valid || res?.success) {
        await invokeTauri('saveGroqKey', { key: apiKey.trim() });
        setKeyVerified(true);
      } else {
        setKeyError(res?.message || 'Invalid Groq API key.');
      }
    } catch (e: any) {
      setKeyError(e?.message || 'Failed to verify API key with Groq.');
    } finally {
      setKeySaving(false);
    }
  };

  const handleFinish = async () => {
    // Confetti celebration burst
    try {
      confetti({
        particleCount: 80,
        spread: 70,
        origin: { y: 0.6 },
        colors: ['#a855f7', '#38bdf8', '#34d399', '#facc15'],
      });
    } catch (e) {}

    await updateSettings({
      push_to_talk_key: selectedKey,
      recording_sounds: soundEnabled,
      onboarding_completed: true,
    });

    await updateProfile({
      onboarding_completed: true,
    });

    setTimeout(() => {
      onComplete();
    }, 600);
  };

  return (
    <AuroraBackground className="w-screen h-screen flex flex-col items-center justify-center bg-[#09090f] relative overflow-hidden px-4">
      {/* Stepper Progress bar */}
      <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginBottom: '20px', zIndex: 10 }}>
        {[1, 2, 3].map((s) => (
          <div
            key={s}
            style={{
              width: s === step ? '32px' : '10px',
              height: '6px',
              borderRadius: '9999px',
              backgroundColor: s === step ? 'var(--accent-purple)' : s < step ? '#34d399' : 'rgba(255, 255, 255, 0.1)',
              transition: 'all 0.3s ease',
            }}
          />
        ))}
      </div>

      {/* Main Container */}
      <SpotlightCard
        spotlightColor="rgba(168, 85, 247, 0.12)"
        style={{
          width: '100%',
          maxWidth: '540px',
          padding: '32px',
          borderRadius: '20px',
          boxShadow: '0 25px 50px -12px rgba(0, 0, 0, 0.7)',
        }}
      >
        {/* ================= STEP 1: WELCOME & AUDIO CHECK ================= */}
        {step === 1 && (
          <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', textAlign: 'center', gap: '16px' }}>
            <div style={{ position: 'relative', margin: '4px 0' }}>
              <AudioOrb size={150} active={true} color="#c084fc" glowColor="#38bdf8" />
            </div>

            <div>
              <Badge variant="violet" size="sm" style={{ marginBottom: '8px' }}>
                <Sparkles className="w-3 h-3 text-purple-300" /> Desktop Audio Engine
              </Badge>
              <h2 style={{ margin: 0, fontSize: '22px', fontWeight: 800, color: '#f0eff4', letterSpacing: '-0.02em' }}>
                Welcome to Rift Dictation
              </h2>
              <p style={{ margin: '8px 0 0', fontSize: '13px', color: '#9c97aa', lineHeight: 1.5, maxWidth: '420px' }}>
                Rift lets you speak naturally and types at 150+ WPM directly into VS Code, Slack, Notion, browsers, and any Windows app with zero latency.
              </p>
            </div>

            <button
              type="button"
              onClick={() => setStep(2)}
              style={{
                marginTop: '12px',
                display: 'flex',
                alignItems: 'center',
                gap: '8px',
                padding: '10px 24px',
                borderRadius: '8px',
                backgroundColor: 'var(--accent-purple)',
                color: '#0c0c12',
                fontSize: '13px',
                fontWeight: 700,
                border: 'none',
                cursor: 'pointer',
              }}
            >
              Get Started <ArrowRight className="w-4 h-4" />
            </button>
          </div>
        )}

        {/* ================= STEP 2: GROQ CLOUD API KEY ================= */}
        {step === 2 && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '18px' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
              <div
                style={{
                  width: '42px',
                  height: '42px',
                  borderRadius: '12px',
                  backgroundColor: 'rgba(52, 211, 153, 0.15)',
                  border: '1px solid rgba(52, 211, 153, 0.3)',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                }}
              >
                <KeyRound style={{ width: '20px', height: '20px', color: '#34d399' }} />
              </div>
              <div>
                <h3 style={{ margin: 0, fontSize: '18px', fontWeight: 700, color: '#f0eff4' }}>
                  Connect Speech Engine
                </h3>
                <span style={{ fontSize: '12px', color: '#9c97aa' }}>
                  Free Whisper LPU transcription with sub-100ms response
                </span>
              </div>
            </div>

            <p style={{ margin: 0, fontSize: '12.5px', color: '#cac4d7', lineHeight: 1.5 }}>
              Rift uses Groq's high-speed LPU infrastructure for instant transcription. Grab a free API key with zero credit card required:
            </p>

            <a
              href="https://console.groq.com/keys"
              target="_blank"
              rel="noreferrer"
              style={{
                display: 'inline-flex',
                alignItems: 'center',
                gap: '6px',
                fontSize: '12px',
                fontWeight: 600,
                color: '#38bdf8',
                textDecoration: 'none',
              }}
            >
              Get free Groq API key from Groq Console <ExternalLink className="w-3.5 h-3.5" />
            </a>

            <div>
              <label style={{ fontSize: '11.5px', color: '#9c97aa', fontWeight: 500, display: 'block', marginBottom: '6px' }}>
                Groq API Key:
              </label>
              <div style={{ display: 'flex', gap: '8px' }}>
                <input
                  type="password"
                  placeholder="gsk_..."
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                  style={{
                    flex: 1,
                    padding: '9px 12px',
                    borderRadius: '8px',
                    backgroundColor: '#101018',
                    border: '1px solid rgba(255, 255, 255, 0.1)',
                    color: '#f0eff4',
                    fontSize: '13px',
                    outline: 'none',
                  }}
                />
                <button
                  type="button"
                  onClick={handleSaveApiKey}
                  disabled={keySaving || keyVerified}
                  style={{
                    padding: '8px 16px',
                    borderRadius: '8px',
                    backgroundColor: keyVerified ? 'rgba(52, 211, 153, 0.2)' : 'var(--accent-purple)',
                    border: `1px solid ${keyVerified ? 'rgba(52, 211, 153, 0.4)' : 'transparent'}`,
                    color: keyVerified ? '#34d399' : '#0c0c12',
                    fontSize: '12px',
                    fontWeight: 600,
                    cursor: keySaving ? 'not-allowed' : 'pointer',
                  }}
                >
                  {keySaving ? 'Testing...' : keyVerified ? 'Connected ✓' : 'Verify'}
                </button>
              </div>

              {keyError && (
                <span style={{ fontSize: '11.5px', color: '#f87171', marginTop: '6px', display: 'block' }}>
                  {keyError}
                </span>
              )}
            </div>

            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginTop: '8px' }}>
              <button
                type="button"
                onClick={() => setStep(3)}
                style={{ background: 'transparent', border: 'none', color: '#9c97aa', fontSize: '12px', cursor: 'pointer' }}
              >
                Skip for now
              </button>

              <button
                type="button"
                onClick={() => setStep(3)}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: '6px',
                  padding: '9px 20px',
                  borderRadius: '8px',
                  backgroundColor: 'var(--accent-purple)',
                  color: '#0c0c12',
                  fontSize: '12.5px',
                  fontWeight: 700,
                  border: 'none',
                  cursor: 'pointer',
                }}
              >
                Continue <ArrowRight className="w-4 h-4" />
              </button>
            </div>
          </div>
        )}

        {/* ================= STEP 3: SHORTCUTS & FINISH ================= */}
        {step === 3 && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '18px' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
              <div
                style={{
                  width: '42px',
                  height: '42px',
                  borderRadius: '12px',
                  backgroundColor: 'rgba(168, 85, 247, 0.15)',
                  border: '1px solid rgba(168, 85, 247, 0.3)',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                }}
              >
                <Keyboard style={{ width: '20px', height: '20px', color: '#c084fc' }} />
              </div>
              <div>
                <h3 style={{ margin: 0, fontSize: '18px', fontWeight: 700, color: '#f0eff4' }}>
                  Choose Your Trigger Key
                </h3>
                <span style={{ fontSize: '12px', color: '#9c97aa' }}>
                  Hold down this key anywhere in Windows to speak
                </span>
              </div>
            </div>

            <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
              {[
                { key: 'Right Alt', label: 'Right Alt (Recommended · Single Thumb Touch)' },
                { key: 'Ctrl+Win', label: 'Ctrl + Win (Safe Combo · Zero Typing Collisions)' },
                { key: 'Right Ctrl', label: 'Right Ctrl (Single Key)' },
              ].map((item) => {
                const isSelected = selectedKey.toLowerCase() === item.key.toLowerCase();
                return (
                  <button
                    key={item.key}
                    type="button"
                    onClick={() => setSelectedKey(item.key)}
                    style={{
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'space-between',
                      padding: '12px 16px',
                      borderRadius: '10px',
                      backgroundColor: isSelected ? 'rgba(168, 85, 247, 0.15)' : '#101018',
                      border: `1px solid ${isSelected ? 'rgba(168, 85, 247, 0.4)' : 'rgba(255, 255, 255, 0.08)'}`,
                      cursor: 'pointer',
                      textAlign: 'left',
                    }}
                  >
                    <span style={{ fontSize: '13px', color: isSelected ? '#f0eff4' : '#cac4d7', fontWeight: 500 }}>
                      {item.label}
                    </span>
                    <Kbd size="md" active={isSelected}>
                      {item.key}
                    </Kbd>
                  </button>
                );
              })}
            </div>

            <button
              type="button"
              onClick={handleFinish}
              style={{
                marginTop: '10px',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                gap: '8px',
                padding: '12px 24px',
                borderRadius: '8px',
                backgroundColor: 'var(--accent-purple)',
                color: '#0c0c12',
                fontSize: '13.5px',
                fontWeight: 800,
                border: 'none',
                cursor: 'pointer',
                boxShadow: '0 4px 14px rgba(168, 85, 247, 0.35)',
              }}
            >
              Start Using Rift <CheckCircle2 className="w-4 h-4" />
            </button>
          </div>
        )}
      </SpotlightCard>
    </AuroraBackground>
  );
};
