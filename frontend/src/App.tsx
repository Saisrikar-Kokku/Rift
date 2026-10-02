import React, { useState } from 'react';
import { useAuth } from './context/AuthContext';
import { useSettings } from './context/SettingsContext';
import { AuthScreen } from './components/auth/AuthScreen';
import { OnboardingWizard } from './components/onboarding/OnboardingWizard';
import { Sidebar, NavigationTab } from './components/layout/Sidebar';
import { Header } from './components/layout/Header';
import { DashboardScreen } from './components/dashboard/DashboardScreen';
import { SettingsScreen } from './components/settings/SettingsScreen';
import { HistoryScreen } from './components/history/HistoryScreen';
import { DictionaryScreen } from './components/dictionary/DictionaryScreen';
import { ShortcutsScreen } from './components/shortcuts/ShortcutsScreen';

export const App: React.FC = () => {
  const { user, loading: authLoading } = useAuth();
  const { settings, loading: settingsLoading } = useSettings();
  const [currentTab, setCurrentTab] = useState<NavigationTab>('home');

  // Loading Splash Screen
  if (authLoading || settingsLoading) {
    return (
      <div
        style={{
          width: '100vw',
          height: '100vh',
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          justifyContent: 'center',
          backgroundColor: '#0c0c12',
          userSelect: 'none',
        }}
      >
        <img
          alt="Rift"
          src="./logo.svg"
          style={{ height: '44px', width: 'auto', marginBottom: '16px', opacity: 0.85 }}
          onError={(e) => {
            (e.currentTarget as HTMLImageElement).style.display = 'none';
          }}
        />
        <span
          style={{
            fontSize: '13px',
            fontWeight: 600,
            color: '#938ea1',
            fontFamily: "'JetBrains Mono', monospace",
          }}
        >
          Loading Rift...
        </span>
      </div>
    );
  }

  // 1. Guard: Authentication via Supabase
  if (!user) {
    return <AuthScreen />;
  }

  // 2. Guard: Sequential Onboarding
  if (settings && (settings as any).onboardingCompleted === false) {
    return <OnboardingWizard onComplete={() => setCurrentTab('home')} />;
  }

  // 3. Main Authenticated Application with Exact Restored Shell
  return (
    <div style={{ minHeight: '100vh', backgroundColor: 'var(--bg-base)', color: 'var(--text-primary)' }}>
      <Sidebar currentTab={currentTab} onSelectTab={setCurrentTab} />
      <Header onOpenSettings={() => setCurrentTab('settings')} />

      {currentTab === 'home' && <DashboardScreen onNavigateToHistory={() => setCurrentTab('history')} />}
      {currentTab === 'history' && <HistoryScreen />}
      {currentTab === 'dictionary' && <DictionaryScreen />}
      {currentTab === 'shortcuts' && <ShortcutsScreen />}
      {currentTab === 'settings' && <SettingsScreen />}
    </div>
  );
};
