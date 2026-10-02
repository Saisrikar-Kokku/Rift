import React, { useState, useEffect, useRef } from 'react';
import { useSettings } from '../../context/SettingsContext';
import { invokeTauri, listenTauriEvent } from '../../lib/tauriBridge';
import { SpotlightCard } from '../ui/SpotlightCard';
import { Badge } from '../ui/Badge';
import { Kbd } from '../ui/Kbd';
import { SimpleTooltip } from '../ui/Tooltip';
import {
  Keyboard,
  RotateCcw,
  AlertCircle,
  Check,
  ShieldCheck,
  Zap,
  Mic,
  Radio,
  Sliders,
  XCircle,
  HelpCircle,
} from 'lucide-react';

export const ShortcutsScreen: React.FC = () => {
  const { settings, updateSetting } = useSettings();
  const [capturingTarget, setCapturingTarget] = useState<'ptt' | 'toggle' | null>(null);
  const [livePreviewChord, setLivePreviewChord] = useState<string>('');
  const [showSavedFeedback, setShowSavedFeedback] = useState(false);

  const pressedModifiersRef = useRef<Set<string>>(new Set());
  const maxChordRef = useRef<string[]>([]);
  const isSavingRef = useRef<boolean>(false);

  const isMac = typeof navigator !== 'undefined' && (/Mac|iPhone|iPod|iPad/i.test(navigator.platform) || /Mac/i.test(navigator.userAgent));

  const formatKeyDisplay = (keyStr: string | null | undefined): string => {
    if (!keyStr) return 'Disabled';
    if (!isMac) return keyStr;
    return keyStr
      .replace(/Right Alt/gi, 'Right Option (⌥)')
      .replace(/Left Alt/gi, 'Left Option (⌥)')
      .replace(/\bAlt\b/gi, 'Option (⌥)')
      .replace(/\bWin\b/gi, 'Cmd (⌘)')
      .replace(/\bCommand\b/gi, 'Cmd (⌘)');
  };

  const s = settings as any;
  const pttMode = s?.push_to_talk_mode || 'single';
  const pttKey = (pttMode === 'combo' ? s?.push_to_talk_combo : s?.push_to_talk_key) || (isMac ? 'Right Option' : 'Right Alt');
  const toggleKey = s?.toggle_recording_key || null;

  const pttPresets = isMac
    ? ['Right Option', 'Cmd+Option', 'Ctrl+Cmd', 'F8', 'F9']
    : ['Right Alt', 'Ctrl+Win', 'Right Ctrl', 'F8', 'F9'];

  const togglePresets = isMac
    ? ['Cmd+Option', 'Ctrl+Cmd', 'Right Option', 'F9', 'F10']
    : ['Ctrl+Win', 'Right Alt', 'Right Ctrl', 'F9', 'F10'];

  // System conflict check
  const systemConflicts = isMac
    ? ['cmd+q', 'cmd+w', 'cmd+c', 'cmd+v', 'cmd+z', 'cmd+x', 'cmd+space']
    : ['win+l', 'ctrl+alt+del', 'alt+f4', 'ctrl+c', 'ctrl+v', 'ctrl+z', 'ctrl+x'];
  const hasPttConflict = systemConflicts.includes(pttKey.toLowerCase().replace(/\s+/g, ''));
  const hasToggleConflict = toggleKey
    ? systemConflicts.includes(toggleKey.toLowerCase().replace(/\s+/g, ''))
    : false;

  const triggerSaved = () => {
    setShowSavedFeedback(true);
    setTimeout(() => setShowSavedFeedback(false), 2000);
  };

  // Helper to commit a shortcut
  const commitShortcut = async (target: 'ptt' | 'toggle', combo: string) => {
    if (!combo || isSavingRef.current) return;
    isSavingRef.current = true;

    try {
      await invokeTauri('cancelHotkeyCapture').catch(() => {});
    } catch (e) {}

    const cleanCombo = combo.trim();
    if (target === 'ptt') {
      const isCombo = cleanCombo.includes('+');
      updateSetting('push_to_talk_key', cleanCombo);
      updateSetting('push_to_talk_mode', isCombo ? 'combo' : 'single');
      if (isCombo) {
        updateSetting('push_to_talk_combo', cleanCombo);
      }
      try {
        await invokeTauri('saveHotkey', { action: 'push_to_talk', combo: cleanCombo });
      } catch (e) {
        console.error('Failed to save PTT hotkey:', e);
      }
    } else {
      updateSetting('toggle_recording_key', cleanCombo);
      try {
        await invokeTauri('saveHotkey', { action: 'toggle_recording', combo: cleanCombo });
      } catch (e) {
        console.error('Failed to save toggle hotkey:', e);
      }
    }

    setCapturingTarget(null);
    setLivePreviewChord('');
    pressedModifiersRef.current.clear();
    maxChordRef.current = [];
    isSavingRef.current = false;
    triggerSaved();
  };

  // 1. Dual-source listener: Global hook capture from backend
  useEffect(() => {
    if (!capturingTarget) return;

    // Start backend native low-level keyboard hook capture
    invokeTauri('startHotkeyCapture', { mode: 'combo' }).catch((err) => {
      console.warn('startHotkeyCapture failed, falling back to window events:', err);
    });

    const unlisten = listenTauriEvent<any>('hotkeyCaptured', (payload) => {
      if (!capturingTarget) return;
      const data = typeof payload === 'string' ? JSON.parse(payload) : payload;
      const combo = data?.combo || data?.key;
      if (combo) {
        commitShortcut(capturingTarget, combo);
      }
    });

    return () => {
      unlisten();
      invokeTauri('cancelHotkeyCapture').catch(() => {});
    };
  }, [capturingTarget]);

  // 2. DOM Keyboard event listener with chord accumulator
  useEffect(() => {
    if (!capturingTarget) return;

    function getModifierLabel(e: KeyboardEvent): string | null {
      if (e.code === 'AltRight' || e.key === 'AltGraph' || (e.key === 'Alt' && e.location === 2)) return 'Right Alt';
      if (e.code === 'AltLeft' || (e.key === 'Alt' && e.location === 1)) return 'Left Alt';
      if (e.code === 'ControlRight' || (e.key === 'Control' && e.location === 2)) return 'Right Ctrl';
      if (e.code === 'ControlLeft' || (e.key === 'Control' && e.location === 1)) return 'Left Ctrl';
      if (e.code === 'ShiftRight' || (e.key === 'Shift' && e.location === 2)) return 'Right Shift';
      if (e.code === 'ShiftLeft' || (e.key === 'Shift' && e.location === 1)) return 'Left Shift';
      if (e.key === 'Meta' || e.key === 'OS' || e.code === 'MetaLeft' || e.code === 'MetaRight') return 'Win';
      return null;
    }

    function getNonModifierLabel(e: KeyboardEvent): string | null {
      if (e.key === 'Escape') return null; // Escape is handled separately
      if (e.code && /^F\d{1,2}$/i.test(e.code)) return e.code.toUpperCase();
      if (e.key && /^F\d{1,2}$/i.test(e.key)) return e.key.toUpperCase();
      if (e.code === 'Space' || e.key === ' ') return 'Space';
      if (e.code === 'Tab' || e.key === 'Tab') return 'Tab';
      if (e.code === 'Enter' || e.key === 'Enter') return 'Enter';
      if (e.code === 'Backspace' || e.key === 'Backspace') return 'Backspace';
      if (e.code === 'CapsLock' || e.key === 'CapsLock') return 'Caps Lock';
      if (e.code === 'Delete' || e.key === 'Delete') return 'Delete';
      if (e.code === 'Pause' || e.key === 'Pause') return 'Pause';
      if (e.code === 'ScrollLock' || e.key === 'ScrollLock') return 'Scroll Lock';
      if (e.code && e.code.startsWith('Key') && e.code.length === 4) return e.code.slice(3).toUpperCase();
      if (e.code && e.code.startsWith('Digit') && e.code.length === 6) return e.code.slice(5);
      if (e.key && e.key.length === 1) return e.key.toUpperCase();
      return e.key || null;
    }

    function formatChord(keys: string[]): string {
      if (!keys || keys.length === 0) return '';
      if (keys.length === 1) return keys[0];

      const hasCtrl = keys.some((k) => k.includes('Ctrl'));
      const hasAlt = keys.some((k) => k.includes('Alt'));
      const hasShift = keys.some((k) => k.includes('Shift'));
      const hasWin = keys.includes('Win');

      const parts: string[] = [];
      if (hasCtrl) parts.push('Ctrl');
      if (hasAlt) parts.push('Alt');
      if (hasShift) parts.push('Shift');
      if (hasWin) parts.push('Win');

      for (const k of keys) {
        if (['Ctrl', 'Left Ctrl', 'Right Ctrl', 'Alt', 'Left Alt', 'Right Alt', 'Shift', 'Left Shift', 'Right Shift', 'Win'].includes(k)) {
          continue;
        }
        if (!parts.includes(k)) parts.push(k);
      }
      return parts.join('+');
    }

    const handleKeyDown = (e: KeyboardEvent) => {
      // Escape alone cancels capture
      if (e.key === 'Escape' && maxChordRef.current.length === 0) {
        e.preventDefault();
        e.stopPropagation();
        setCapturingTarget(null);
        setLivePreviewChord('');
        invokeTauri('cancelHotkeyCapture').catch(() => {});
        return;
      }

      e.preventDefault();
      e.stopPropagation();

      const mod = getModifierLabel(e);
      if (mod) {
        pressedModifiersRef.current.add(mod);
        if (!maxChordRef.current.includes(mod)) {
          maxChordRef.current.push(mod);
        }
        setLivePreviewChord(formatChord(maxChordRef.current));
        return;
      }

      const nonMod = getNonModifierLabel(e);
      if (nonMod) {
        if (!maxChordRef.current.includes(nonMod)) {
          maxChordRef.current.push(nonMod);
        }
        const combo = formatChord(maxChordRef.current);
        commitShortcut(capturingTarget, combo);
      }
    };

    const handleKeyUp = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();

      const mod = getModifierLabel(e);
      if (mod) {
        pressedModifiersRef.current.delete(mod);
      }

      // If user pressed only modifiers (e.g. single Right Alt or Ctrl+Win) and released all keys
      if (pressedModifiersRef.current.size === 0 && maxChordRef.current.length > 0) {
        const combo = formatChord(maxChordRef.current);
        if (combo) {
          commitShortcut(capturingTarget, combo);
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown, true);
    window.addEventListener('keyup', handleKeyUp, true);

    return () => {
      window.removeEventListener('keydown', handleKeyDown, true);
      window.removeEventListener('keyup', handleKeyUp, true);
    };
  }, [capturingTarget]);

  // Quick Assignment Actions
  const handleAssignPtt = (key: string) => {
    commitShortcut('ptt', key);
  };

  const handleAssignToggle = (key: string) => {
    commitShortcut('toggle', key);
  };

  const handleClearToggle = async () => {
    updateSetting('toggle_recording_key', null);
    try {
      await invokeTauri('clearHotkey', { action: 'toggle_recording' });
    } catch (e) {
      console.error('Failed to clear toggle hotkey:', e);
    }
    triggerSaved();
  };

  const handleRestoreDefaults = async () => {
    updateSetting('push_to_talk_key', 'Right Alt');
    updateSetting('push_to_talk_mode', 'single');
    updateSetting('push_to_talk_combo', 'Ctrl+Win');
    updateSetting('toggle_recording_key', null);
    try {
      await invokeTauri('saveHotkey', { action: 'push_to_talk', combo: 'Right Alt' });
      await invokeTauri('clearHotkey', { action: 'toggle_recording' });
    } catch (e) {}
    triggerSaved();
  };

  const fKeyOptions = Array.from({ length: 24 }, (_, i) => `F${i + 1}`);

  return (
    <div className="rift-main-area">
      <div style={{ maxWidth: '1040px', margin: '0 auto', display: 'flex', flexDirection: 'column', gap: '22px' }}>
        
        {/* Header Toolbar */}
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', flexWrap: 'wrap', gap: '14px' }}>
          <div>
            <h1 style={{ margin: 0, fontSize: '24px', fontWeight: 700, color: '#f0eff4', letterSpacing: '-0.02em' }}>
              Keyboard Shortcuts & Triggers
            </h1>
            <p style={{ margin: '4px 0 0', fontSize: '13px', color: '#9c97aa' }}>
              Low-level Windows keyboard hook (`WH_KEYBOARD_LL`) with hardware zero-latency activation.
            </p>
          </div>

          <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
            <div className={`save-indicator-badge ${showSavedFeedback ? 'visible' : ''}`}>
              <Check style={{ width: '13px', height: '13px' }} />
              <span>Shortcut updated</span>
            </div>

            <button
              type="button"
              onClick={handleRestoreDefaults}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: '6px',
                padding: '7px 12px',
                borderRadius: '8px',
                backgroundColor: '#161622',
                border: '1px solid rgba(255, 255, 255, 0.08)',
                color: '#cac4d7',
                fontSize: '12px',
                cursor: 'pointer',
                transition: 'all 0.15s ease',
              }}
            >
              <RotateCcw style={{ width: '13px', height: '13px' }} />
              Reset Defaults
            </button>
          </div>
        </div>

        {/* ================= PRIMARY PUSH-TO-TALK STUDIO CARD ================= */}
        <SpotlightCard
          spotlightColor="rgba(167, 139, 250, 0.15)"
          style={{
            padding: '24px',
            display: 'flex',
            flexDirection: 'column',
            gap: '20px',
            borderColor: capturingTarget === 'ptt' ? 'rgba(167, 139, 250, 0.5)' : undefined,
          }}
        >
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', flexWrap: 'wrap', gap: '14px' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '14px' }}>
              <div
                style={{
                  width: '46px',
                  height: '46px',
                  borderRadius: '12px',
                  backgroundColor: 'rgba(167, 139, 250, 0.15)',
                  border: '1px solid rgba(167, 139, 250, 0.3)',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                }}
              >
                <Keyboard style={{ width: '22px', height: '22px', color: 'var(--accent-purple)' }} />
              </div>
              <div>
                <span style={{ fontSize: '16px', fontWeight: 600, color: '#f0eff4', display: 'block' }}>
                  Primary Push-to-Talk Shortcut
                </span>
                <span style={{ fontSize: '12.5px', color: '#9c97aa' }}>
                  Press and hold this key to dictate. Speech is automatically pasted upon release.
                </span>
              </div>
            </div>

            {/* Current Key Display & Recorder Button */}
            <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
              <Kbd size="lg" active={capturingTarget === 'ptt'}>
                {capturingTarget === 'ptt'
                  ? (livePreviewChord || 'Press Any Key...')
                  : formatKeyDisplay(pttKey)}
              </Kbd>

              <button
                type="button"
                onClick={() => {
                  if (capturingTarget === 'ptt') {
                    setCapturingTarget(null);
                    setLivePreviewChord('');
                    invokeTauri('cancelHotkeyCapture').catch(() => {});
                  } else {
                    setCapturingTarget('ptt');
                    setLivePreviewChord('');
                  }
                }}
                style={{
                  padding: '8px 16px',
                  borderRadius: '8px',
                  backgroundColor: capturingTarget === 'ptt' ? 'rgba(239, 68, 68, 0.2)' : 'var(--accent-purple)',
                  border: `1px solid ${capturingTarget === 'ptt' ? 'rgba(239, 68, 68, 0.4)' : 'transparent'}`,
                  color: capturingTarget === 'ptt' ? '#f87171' : '#0c0c12',
                  fontSize: '12.5px',
                  fontWeight: 600,
                  cursor: 'pointer',
                  transition: 'all 0.15s ease',
                }}
              >
                {capturingTarget === 'ptt' ? 'Cancel (Esc)' : 'Record Key'}
              </button>
            </div>
          </div>

          {/* Conflict Warning or Safe Indicator */}
          {hasPttConflict ? (
            <div style={{ display: 'flex', alignItems: 'center', gap: '8px', padding: '10px 14px', borderRadius: '8px', backgroundColor: 'rgba(239, 68, 68, 0.1)', border: '1px solid rgba(239, 68, 68, 0.25)', color: '#f87171', fontSize: '12px' }}>
              <AlertCircle style={{ width: '16px', height: '16px' }} />
              Warning: "{formatKeyDisplay(pttKey)}" conflicts with a reserved system shortcut. We recommend using {isMac ? 'Right Option or Cmd+Option' : 'Right Alt or Ctrl+Win'}.
            </div>
          ) : (
            <div style={{ display: 'flex', alignItems: 'center', gap: '8px', fontSize: '12px', color: '#45dfa9' }}>
              <ShieldCheck style={{ width: '15px', height: '15px' }} />
              Zero {isMac ? 'macOS' : 'Windows'} shortcut conflicts detected. Safe for global low-latency use.
            </div>
          )}

          {/* Quick Presets & F-Key Selector */}
          <div style={{ borderTop: '1px solid rgba(255, 255, 255, 0.06)', paddingTop: '16px', display: 'flex', alignItems: 'center', justifyContent: 'space-between', flexWrap: 'wrap', gap: '12px' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '8px', flexWrap: 'wrap' }}>
              <span style={{ fontSize: '12px', color: '#9c97aa', fontWeight: 500 }}>
                Recommended Presets:
              </span>
              <div style={{ display: 'flex', gap: '6px', flexWrap: 'wrap' }}>
                {pttPresets.map((preset) => {
                  const isSelected = pttKey.toLowerCase() === preset.toLowerCase();
                  return (
                    <button
                      key={preset}
                      type="button"
                      onClick={() => handleAssignPtt(preset)}
                      style={{
                        display: 'flex',
                        alignItems: 'center',
                        gap: '5px',
                        padding: '5px 10px',
                        borderRadius: '6px',
                        backgroundColor: isSelected ? 'rgba(167, 139, 250, 0.2)' : '#161622',
                        border: `1px solid ${isSelected ? 'rgba(167, 139, 250, 0.4)' : 'rgba(255, 255, 255, 0.08)'}`,
                        color: isSelected ? '#c4b5fd' : '#cac4d7',
                        fontSize: '11.5px',
                        fontWeight: 500,
                        cursor: 'pointer',
                        transition: 'all 0.15s ease',
                      }}
                    >
                      <span>{formatKeyDisplay(preset)}</span>
                      {isSelected && <Check style={{ width: '12px', height: '12px' }} />}
                    </button>
                  );
                })}
              </div>
            </div>

            {/* Function Keys Selector */}
            <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
              <span style={{ fontSize: '12px', color: '#9c97aa' }}>Function Key:</span>
              <select
                className="setting-select"
                style={{ padding: '4px 10px', fontSize: '12px', minWidth: '85px', height: '30px' }}
                value={pttKey.startsWith('F') ? pttKey : ''}
                onChange={(e) => {
                  if (e.target.value) handleAssignPtt(e.target.value);
                }}
              >
                <option value="">Select...</option>
                {fKeyOptions.map((f) => (
                  <option key={f} value={f}>
                    {f}
                  </option>
                ))}
              </select>
            </div>
          </div>
        </SpotlightCard>

        {/* ================= SECONDARY TOGGLE RECORDING CARD ================= */}
        <SpotlightCard
          spotlightColor="rgba(56, 189, 248, 0.12)"
          style={{
            padding: '22px 24px',
            display: 'flex',
            flexDirection: 'column',
            gap: '16px',
            borderColor: capturingTarget === 'toggle' ? 'rgba(56, 189, 248, 0.5)' : undefined,
          }}
        >
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', flexWrap: 'wrap', gap: '14px' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '14px' }}>
              <div
                style={{
                  width: '42px',
                  height: '42px',
                  borderRadius: '10px',
                  backgroundColor: 'rgba(56, 189, 248, 0.12)',
                  border: '1px solid rgba(56, 189, 248, 0.25)',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                }}
              >
                <Radio style={{ width: '20px', height: '20px', color: '#38bdf8' }} />
              </div>
              <div>
                <span style={{ fontSize: '15px', fontWeight: 600, color: '#f0eff4', display: 'block' }}>
                  Hands-Free Toggle Mode (Start / Stop)
                </span>
                <span style={{ fontSize: '12.5px', color: '#9c97aa' }}>
                  Press once to start continuous recording, press again to stop and automatically inject text.
                </span>
              </div>
            </div>

            <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
              <Kbd size="md" active={capturingTarget === 'toggle'}>
                {capturingTarget === 'toggle'
                  ? (livePreviewChord || 'Press Key...')
                  : formatKeyDisplay(toggleKey)}
              </Kbd>

              <button
                type="button"
                onClick={() => {
                  if (capturingTarget === 'toggle') {
                    setCapturingTarget(null);
                    setLivePreviewChord('');
                    invokeTauri('cancelHotkeyCapture').catch(() => {});
                  } else {
                    setCapturingTarget('toggle');
                    setLivePreviewChord('');
                  }
                }}
                style={{
                  padding: '7px 14px',
                  borderRadius: '7px',
                  backgroundColor: capturingTarget === 'toggle' ? 'rgba(239, 68, 68, 0.2)' : '#161622',
                  border: `1px solid ${capturingTarget === 'toggle' ? 'rgba(239, 68, 68, 0.4)' : 'rgba(255, 255, 255, 0.1)'}`,
                  color: capturingTarget === 'toggle' ? '#f87171' : '#f0eff4',
                  fontSize: '12px',
                  fontWeight: 600,
                  cursor: 'pointer',
                  transition: 'all 0.15s ease',
                }}
              >
                {capturingTarget === 'toggle' ? 'Cancel (Esc)' : (toggleKey ? 'Change Key' : 'Assign Key')}
              </button>

              {toggleKey && (
                <SimpleTooltip content="Disable Hands-Free Toggle Mode">
                  <button
                    type="button"
                    onClick={handleClearToggle}
                    style={{
                      padding: '7px 10px',
                      borderRadius: '7px',
                      backgroundColor: 'rgba(239, 68, 68, 0.1)',
                      border: '1px solid rgba(239, 68, 68, 0.2)',
                      color: '#f87171',
                      fontSize: '12px',
                      fontWeight: 500,
                      cursor: 'pointer',
                      display: 'flex',
                      alignItems: 'center',
                      gap: '4px',
                    }}
                  >
                    <XCircle style={{ width: '13px', height: '13px' }} />
                    <span>Disable</span>
                  </button>
                </SimpleTooltip>
              )}
            </div>
          </div>

          {/* Toggle Presets */}
          <div style={{ borderTop: '1px solid rgba(255, 255, 255, 0.06)', paddingTop: '14px', display: 'flex', alignItems: 'center', justifyContent: 'space-between', flexWrap: 'wrap', gap: '12px' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: '8px', flexWrap: 'wrap' }}>
              <span style={{ fontSize: '12px', color: '#9c97aa', fontWeight: 500 }}>
                Popular Toggle Keys:
              </span>
              <div style={{ display: 'flex', gap: '6px', flexWrap: 'wrap' }}>
                {togglePresets.map((preset) => {
                  const isSelected = toggleKey && toggleKey.toLowerCase() === preset.toLowerCase();
                  return (
                    <button
                      key={preset}
                      type="button"
                      onClick={() => handleAssignToggle(preset)}
                      style={{
                        display: 'flex',
                        alignItems: 'center',
                        gap: '5px',
                        padding: '4px 9px',
                        borderRadius: '6px',
                        backgroundColor: isSelected ? 'rgba(56, 189, 248, 0.2)' : '#161622',
                        border: `1px solid ${isSelected ? 'rgba(56, 189, 248, 0.4)' : 'rgba(255, 255, 255, 0.08)'}`,
                        color: isSelected ? '#38bdf8' : '#cac4d7',
                        fontSize: '11.5px',
                        fontWeight: 500,
                        cursor: 'pointer',
                        transition: 'all 0.15s ease',
                      }}
                    >
                      <span>{formatKeyDisplay(preset)}</span>
                      {isSelected && <Check style={{ width: '12px', height: '12px' }} />}
                    </button>
                  );
                })}
              </div>
            </div>

            {/* Function Keys Selector for Toggle */}
            <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
              <span style={{ fontSize: '12px', color: '#9c97aa' }}>Function Key:</span>
              <select
                className="setting-select"
                style={{ padding: '4px 10px', fontSize: '12px', minWidth: '85px', height: '30px' }}
                value={toggleKey && toggleKey.startsWith('F') ? toggleKey : ''}
                onChange={(e) => {
                  if (e.target.value) handleAssignToggle(e.target.value);
                }}
              >
                <option value="">Select...</option>
                {fKeyOptions.map((f) => (
                  <option key={f} value={f}>
                    {f}
                  </option>
                ))}
              </select>
            </div>
          </div>
        </SpotlightCard>

        {/* ================= SYSTEM ARCHITECTURE CARD ================= */}
        <SpotlightCard
          spotlightColor="rgba(255, 255, 255, 0.03)"
          style={{ padding: '20px 24px', display: 'flex', flexDirection: 'column', gap: '12px' }}
        >
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <Zap style={{ width: '16px', height: '16px', color: '#facc15' }} />
            <h3 style={{ margin: 0, fontSize: '14.5px', fontWeight: 600, color: '#f0eff4' }}>
              Windows Low-Level Keyboard Architecture (`WH_KEYBOARD_LL`)
            </h3>
          </div>
          <p style={{ margin: 0, fontSize: '12.5px', color: '#9c97aa', lineHeight: 1.6 }}>
            Rift registers a dedicated background OS thread using <code style={{ color: '#c4b5fd', fontFamily: 'monospace' }}>SetWindowsHookExW(WH_KEYBOARD_LL)</code>. This intercepts key-down and key-up signals at 0ms latency without injecting spurious key codes into your active window, ensuring 100% compatibility with full-screen games, terminals, and IDEs.
          </p>
        </SpotlightCard>
      </div>
    </div>
  );
};
