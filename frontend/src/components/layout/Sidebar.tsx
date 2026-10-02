import React from 'react';
import { useAuth } from '../../context/AuthContext';

export type NavigationTab = 'home' | 'history' | 'dictionary' | 'shortcuts' | 'settings';

interface SidebarProps {
  currentTab: NavigationTab;
  onSelectTab: (tab: NavigationTab) => void;
}

export const Sidebar: React.FC<SidebarProps> = ({ currentTab, onSelectTab }) => {
  const { user, profile, signOut } = useAuth();

  const navItems: { id: NavigationTab; label: string; icon: string }[] = [
    { id: 'home', label: 'Home', icon: 'home' },
    { id: 'history', label: 'History', icon: 'schedule' },
    { id: 'dictionary', label: 'Dictionary', icon: 'book_2' },
    { id: 'shortcuts', label: 'Shortcuts', icon: 'keyboard' },
    { id: 'settings', label: 'Settings', icon: 'settings' },
  ];

  return (
    <aside className="rift-sidebar">
      <div style={{ display: 'flex', flexDirection: 'column', gap: '20px' }}>
        {/* Brand / Logo */}
        <div style={{ display: 'flex', alignItems: 'center', gap: '12px', padding: '4px 8px' }}>
          <img
            alt="Rift Logo"
            src="./logo.svg"
            style={{ height: '30px', width: 'auto', objectFit: 'contain' }}
            onError={(e) => {
              // fallback if svg not loaded
              (e.currentTarget as HTMLImageElement).style.display = 'none';
            }}
          />
          <span style={{ fontSize: '18px', fontWeight: 700, color: '#e4e1ea', letterSpacing: '-0.02em' }}>
            Rift
          </span>
        </div>

        {/* Navigation Items */}
        <nav style={{ display: 'flex', flexDirection: 'column', gap: '4px' }}>
          {navItems.map((item) => {
            const isActive = currentTab === item.id;
            return (
              <button
                key={item.id}
                onClick={() => onSelectTab(item.id)}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: '10px',
                  padding: '10px 14px',
                  borderRadius: '10px',
                  backgroundColor: isActive ? '#1f1f28' : 'transparent',
                  color: isActive ? '#e4e1ea' : '#938ea1',
                  fontSize: '14px',
                  fontWeight: isActive ? 600 : 500,
                  border: 'none',
                  cursor: 'pointer',
                  textAlign: 'left',
                  width: '100%',
                  transition: 'color 0.15s, background 0.15s',
                }}
                onMouseEnter={(e) => {
                  if (!isActive) {
                    e.currentTarget.style.backgroundColor = '#1a1a24';
                    e.currentTarget.style.color = '#e4e1ea';
                  }
                }}
                onMouseLeave={(e) => {
                  if (!isActive) {
                    e.currentTarget.style.backgroundColor = 'transparent';
                    e.currentTarget.style.color = '#938ea1';
                  }
                }}
              >
                <span
                  className="material-symbols-outlined"
                  style={{
                    fontSize: '19px',
                    color: isActive ? '#cabeff' : 'inherit',
                  }}
                >
                  {item.icon}
                </span>
                <span>{item.label}</span>
              </button>
            );
          })}
        </nav>
      </div>

      {/* Footer Area: User Status, Version & Engine Status */}
      <div style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
        {/* Supabase User & Logout */}
        {user && (
          <div
            style={{
              padding: '8px 10px',
              borderRadius: '8px',
              backgroundColor: '#161622',
              border: '1px solid rgba(255,255,255,0.06)',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              gap: '6px',
            }}
          >
            <div style={{ display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
              <span
                style={{
                  fontSize: '11px',
                  fontWeight: 600,
                  color: '#e4e1ea',
                  whiteSpace: 'nowrap',
                  overflow: 'hidden',
                  textOverflow: 'ellipsis',
                }}
              >
                {profile?.full_name || user.email?.split('@')[0] || 'User'}
              </span>
              <span
                style={{
                  fontSize: '9.5px',
                  color: '#938ea1',
                  whiteSpace: 'nowrap',
                  overflow: 'hidden',
                  textOverflow: 'ellipsis',
                }}
              >
                {user.email}
              </span>
            </div>
            <button
              onClick={() => signOut()}
              title="Sign Out"
              style={{
                background: 'transparent',
                border: 'none',
                color: '#938ea1',
                cursor: 'pointer',
                padding: '4px',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                borderRadius: '4px',
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.color = '#f87171';
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.color = '#938ea1';
              }}
            >
              <span className="material-symbols-outlined" style={{ fontSize: '15px' }}>
                logout
              </span>
            </button>
          </div>
        )}

        {/* Engine Ready Indicator */}
        <div
          style={{
            paddingTop: '12px',
            borderTop: '1px solid rgba(255,255,255,0.08)',
            paddingLeft: '6px',
            paddingRight: '6px',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
          }}
        >
          <span style={{ fontFamily: "'JetBrains Mono', monospace", fontSize: '11px', color: '#938ea1' }}>
            v1.0.0
          </span>
          <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }} title="Engine Ready">
            <span
              style={{
                height: '8px',
                width: '8px',
                borderRadius: '50%',
                backgroundColor: '#45dfa9',
                boxShadow: '0 0 8px rgba(69,223,169,0.8)',
              }}
            />
            <span style={{ fontSize: '11px', color: '#938ea1', fontFamily: "'JetBrains Mono', monospace" }}>
              Ready
            </span>
          </div>
        </div>
      </div>
    </aside>
  );
};
