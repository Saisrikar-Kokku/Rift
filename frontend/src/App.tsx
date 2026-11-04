import React, { lazy, Suspense, useEffect, useState } from 'react';
import { useAuth } from './context/AuthContext';
import { useSettings } from './context/SettingsContext';
import { Sidebar, NavigationTab } from './components/layout/Sidebar';
import { Header } from './components/layout/Header';
import { DashboardScreen } from './components/dashboard/DashboardScreen';
import { isSupabaseConfigured } from './lib/supabase';
import { listenTauriEvent } from './lib/tauriBridge';

const AuthScreen = lazy(() => import('./components/auth/AuthScreen').then((module) => ({ default: module.AuthScreen })));
const OnboardingWizard = lazy(() => import('./components/onboarding/OnboardingWizard').then((module) => ({ default: module.OnboardingWizard })));
const HistoryScreen = lazy(() => import('./components/history/HistoryScreen').then((module) => ({ default: module.HistoryScreen })));
const SettingsScreen = lazy(() => import('./components/settings/SettingsScreen').then((module) => ({ default: module.SettingsScreen })));
const LibraryScreen = lazy(() => import('./components/library/LibraryScreen').then((module) => ({ default: module.LibraryScreen })));
const DictionaryScreen = lazy(() => import('./components/dictionary/DictionaryScreen').then((module) => ({ default: module.DictionaryScreen })));
const ShortcutsScreen = lazy(() => import('./components/shortcuts/ShortcutsScreen').then((module) => ({ default: module.ShortcutsScreen })));

const navigationTabs: NavigationTab[] = ['home', 'library', 'history', 'dictionary', 'shortcuts', 'settings'];

function isNavigationTab(value: unknown): value is NavigationTab {
  return typeof value === 'string' && navigationTabs.includes(value as NavigationTab);
}

export const App: React.FC = () => {
  const { user, loading: authLoading } = useAuth();
  const { settings, loading: settingsLoading } = useSettings();
  const [currentTab, setCurrentTab] = useState<NavigationTab>('home');

  useEffect(() => listenTauriEvent<unknown>('navigateTo', (destination) => {
    if (isNavigationTab(destination)) setCurrentTab(destination);
  }), []);

  useEffect(() => {
    const handleSearchShortcut = (event: KeyboardEvent) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k' && !event.altKey) {
        event.preventDefault();
        setCurrentTab('library');
        window.setTimeout(() => {
          window.dispatchEvent(new Event('rift:focus-library-search'));
        }, 0);
      }
    };

    window.addEventListener('keydown', handleSearchShortcut);
    return () => window.removeEventListener('keydown', handleSearchShortcut);
  }, []);

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
  if (isSupabaseConfigured && !user) {
    return <Suspense fallback={<RouteLoading />}><AuthScreen /></Suspense>;
  }

  // 2. Guard: Sequential Onboarding
  if (settings && (settings as any).onboardingCompleted === false) {
    return <Suspense fallback={<RouteLoading />}><OnboardingWizard onComplete={() => setCurrentTab('home')} /></Suspense>;
  }

  // 3. Main Authenticated Application with Exact Restored Shell
  return (
    <div style={{ minHeight: '100vh', backgroundColor: 'var(--bg-base)', color: 'var(--text-primary)' }}>
      <Sidebar currentTab={currentTab} onSelectTab={setCurrentTab} />
      <Header onOpenSettings={() => setCurrentTab('settings')} />

      <Suspense fallback={<RouteLoading />}>
        {currentTab === 'home' && <DashboardScreen onNavigateToLibrary={() => setCurrentTab('library')} />}
        {currentTab === 'library' && <LibraryScreen />}
        {currentTab === 'history' && <HistoryScreen onOpenLibrary={() => setCurrentTab('library')} />}
        {currentTab === 'dictionary' && <DictionaryScreen />}
        {currentTab === 'shortcuts' && <ShortcutsScreen />}
        {currentTab === 'settings' && <SettingsScreen />}
      </Suspense>
    </div>
  );
};

const RouteLoading: React.FC = () => (
  <div role="status" aria-live="polite" style={{ minHeight: '160px', display: 'grid', placeItems: 'center', color: 'var(--text-secondary, #9c97aa)', fontSize: '13px' }}>
    Loading…
  </div>
);
