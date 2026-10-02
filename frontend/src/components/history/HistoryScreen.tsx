import React, { useState, useEffect, useMemo, useCallback } from 'react';
import { invokeTauri, listenTauriEvent } from '../../lib/tauriBridge';
import { SpotlightCard } from '../ui/SpotlightCard';
import { Badge } from '../ui/Badge';
import { SimpleTooltip } from '../ui/Tooltip';
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '../ui/Dialog';
import { Search, Trash2, Copy, Check, Calendar, Clock, AlertTriangle, FileText } from 'lucide-react';

interface HistoryEntry {
  id: number;
  timestamp: string;
  text: string;
  duration_seconds: number;
  char_count?: number;
  words_count: number;
  target_app?: string;
  model?: string;
  engine?: string;
}

export const HistoryScreen: React.FC = () => {
  const [history, setHistory] = useState<HistoryEntry[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [activeFilter, setActiveFilter] = useState<'all' | 'today' | 'week' | 'cloud' | 'local'>('all');
  const [copiedId, setCopiedId] = useState<number | null>(null);
  const [showClearConfirmDialog, setShowClearConfirmDialog] = useState(false);

  const loadHistory = useCallback(async () => {
    try {
      const items = await invokeTauri<HistoryEntry[]>('getHistory', { filter: { limit: 120 } });
      if (Array.isArray(items)) {
        setHistory(items);
      }
    } catch (e) {
      console.warn('Failed to load history', e);
    }
  }, []);

  useEffect(() => {
    loadHistory();

    const u1 = listenTauriEvent('historyCleared', loadHistory);
    const u2 = listenTauriEvent('transcriptionSuccess', loadHistory);
    const u3 = listenTauriEvent('transcriptionComplete', loadHistory);

    return () => {
      u1();
      u2();
      u3();
    };
  }, [loadHistory]);

  const handleCopy = async (id: number, text: string) => {
    try {
      await invokeTauri('copyToClipboard', { text });
      navigator.clipboard.writeText(text);
      setCopiedId(id);
      setTimeout(() => setCopiedId(null), 1800);
    } catch (err) {
      console.error('Failed to copy', err);
    }
  };

  const handleDelete = async (id: number, e: React.MouseEvent) => {
    e.stopPropagation();
    try {
      await invokeTauri('deleteHistoryEntry', { id });
      setHistory((prev) => prev.filter((item) => item.id !== id));
    } catch (err) {
      console.error('Failed to delete history item', err);
    }
  };

  const handleConfirmClear = async () => {
    try {
      await invokeTauri('clearHistory');
      setHistory([]);
      setShowClearConfirmDialog(false);
    } catch (err) {
      console.error('Failed to clear history', err);
    }
  };

  // Filter items
  const filteredHistory = useMemo(() => {
    const q = searchQuery.toLowerCase().trim();
    const now = new Date();

    return history.filter((item) => {
      if (q && !item.text.toLowerCase().includes(q)) {
        return false;
      }

      if (activeFilter === 'all') return true;

      const itemDate = new Date(item.timestamp);
      if (activeFilter === 'today') {
        return itemDate.toDateString() === now.toDateString();
      }
      if (activeFilter === 'week') {
        const oneWeekAgo = new Date();
        oneWeekAgo.setDate(now.getDate() - 7);
        return itemDate >= oneWeekAgo;
      }
      if (activeFilter === 'cloud') {
        return !item.engine || item.engine.toLowerCase() !== 'local';
      }
      if (activeFilter === 'local') {
        return item.engine && item.engine.toLowerCase() === 'local';
      }
      return true;
    });
  }, [history, searchQuery, activeFilter]);

  function formatTime(isoString: string) {
    if (!isoString) return '';
    try {
      const d = new Date(isoString);
      return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    } catch {
      return '';
    }
  }

  function formatDate(isoString: string) {
    if (!isoString) return '';
    try {
      const d = new Date(isoString);
      const today = new Date();
      if (d.toDateString() === today.toDateString()) return 'Today';
      const yesterday = new Date();
      yesterday.setDate(today.getDate() - 1);
      if (d.toDateString() === yesterday.toDateString()) return 'Yesterday';
      return d.toLocaleDateString([], { month: 'short', day: 'numeric' });
    } catch {
      return '';
    }
  }

  const filterTabs = [
    { id: 'all', label: 'All Time' },
    { id: 'today', label: 'Today' },
    { id: 'week', label: 'Past 7 Days' },
    { id: 'cloud', label: 'Cloud' },
    { id: 'local', label: 'Offline Whisper' },
  ] as const;

  return (
    <div className="rift-main-area">
      <div style={{ maxWidth: '1040px', margin: '0 auto', display: 'flex', flexDirection: 'column', gap: '20px' }}>
        
        {/* Header toolbar */}
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', flexWrap: 'wrap', gap: '14px' }}>
          <div>
            <h1 style={{ margin: 0, fontSize: '24px', fontWeight: 700, color: '#f0eff4', letterSpacing: '-0.02em' }}>
              Dictation History
            </h1>
            <p style={{ margin: '4px 0 0', fontSize: '13px', color: '#9c97aa' }}>
              Search, review, and copy transcriptions from your local SQLite archive.
            </p>
          </div>

          {history.length > 0 && (
            <button
              type="button"
              onClick={() => setShowClearConfirmDialog(true)}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: '6px',
                padding: '7px 12px',
                borderRadius: '8px',
                backgroundColor: 'rgba(239, 68, 68, 0.10)',
                border: '1px solid rgba(239, 68, 68, 0.25)',
                color: '#f87171',
                fontSize: '12px',
                fontWeight: 600,
                cursor: 'pointer',
                transition: 'all 0.15s ease',
              }}
              onMouseEnter={(e) => {
                e.currentTarget.style.backgroundColor = 'rgba(239, 68, 68, 0.20)';
              }}
              onMouseLeave={(e) => {
                e.currentTarget.style.backgroundColor = 'rgba(239, 68, 68, 0.10)';
              }}
            >
              <Trash2 style={{ width: '14px', height: '14px' }} />
              Clear History
            </button>
          )}
        </div>

        {/* Search & Filter Pills Row */}
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', flexWrap: 'wrap', gap: '12px' }}>
          
          {/* Search Box */}
          <div style={{ position: 'relative', width: '320px', maxWidth: '100%' }}>
            <Search
              style={{
                position: 'absolute',
                left: '12px',
                top: '50%',
                transform: 'translateY(-50%)',
                width: '15px',
                height: '15px',
                color: '#6e6a7d',
              }}
            />
            <input
              type="text"
              placeholder="Search transcriptions..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              style={{
                width: '100%',
                padding: '9px 12px 9px 36px',
                borderRadius: '9px',
                backgroundColor: '#13131c',
                border: '1px solid rgba(255, 255, 255, 0.08)',
                color: '#f0eff4',
                fontSize: '12.5px',
                outline: 'none',
                boxSizing: 'border-box',
                transition: 'border-color 0.15s ease',
              }}
              onFocus={(e) => (e.target.style.borderColor = 'rgba(167, 139, 250, 0.4)')}
              onBlur={(e) => (e.target.style.borderColor = 'rgba(255, 255, 255, 0.08)')}
            />
          </div>

          {/* Filter Pills */}
          <div style={{ display: 'flex', gap: '6px', background: '#101018', padding: '3px', borderRadius: '9px' }}>
            {filterTabs.map((tab) => {
              const isActive = activeFilter === tab.id;
              return (
                <button
                  key={tab.id}
                  type="button"
                  onClick={() => setActiveFilter(tab.id as any)}
                  style={{
                    padding: '5px 12px',
                    borderRadius: '7px',
                    border: 'none',
                    backgroundColor: isActive ? 'rgba(167, 139, 250, 0.15)' : 'transparent',
                    color: isActive ? '#c4b5fd' : '#9c97aa',
                    fontSize: '12px',
                    fontWeight: isActive ? 600 : 500,
                    cursor: 'pointer',
                    transition: 'all 0.15s ease',
                  }}
                >
                  {tab.label}
                </button>
              );
            })}
          </div>
        </div>

        {/* History Items List */}
        {filteredHistory.length === 0 ? (
          <SpotlightCard style={{ padding: '60px 24px', textAlign: 'center' }}>
            <FileText style={{ width: '36px', height: '36px', color: '#6e6a7d', margin: '0 auto 12px' }} />
            <h3 style={{ margin: 0, fontSize: '16px', fontWeight: 600, color: '#f0eff4' }}>
              No dictations found
            </h3>
            <p style={{ margin: '6px 0 0', fontSize: '13px', color: '#9c97aa' }}>
              {searchQuery ? 'Try adjusting your search query or active filter.' : 'Your transcribed recordings will appear here.'}
            </p>
          </SpotlightCard>
        ) : (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
            {filteredHistory.map((item) => {
              const isCopied = copiedId === item.id;
              return (
                <SpotlightCard
                  key={item.id}
                  spotlightColor="rgba(167, 139, 250, 0.08)"
                  style={{
                    padding: '16px 20px',
                    display: 'flex',
                    flexDirection: 'column',
                    gap: '12px',
                  }}
                >
                  {/* Top metadata row */}
                  <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '10px', fontSize: '11.5px', color: '#9c97aa' }}>
                      <span style={{ display: 'flex', alignItems: 'center', gap: '4px', fontWeight: 600, color: '#cac4d7' }}>
                        <Calendar style={{ width: '13px', height: '13px' }} />
                        {formatDate(item.timestamp)}
                      </span>
                      <span>•</span>
                      <span style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
                        <Clock style={{ width: '13px', height: '13px' }} />
                        {formatTime(item.timestamp)}
                      </span>
                      <Badge variant="subtle">
                        {item.words_count || (item.text ? item.text.trim().split(/\s+/).filter(Boolean).length : 0)} words
                      </Badge>
                      {item.duration_seconds > 0 && (
                        <Badge variant="subtle">{item.duration_seconds.toFixed(1)}s</Badge>
                      )}
                      {item.model && (
                        <Badge variant="outline" className="font-mono text-[10px] text-zinc-400">
                          {item.model.replace(/^groq\//, '').replace(/^openai\//, '').replace(/^microsoft\//, '')}
                        </Badge>
                      )}
                    </div>

                    <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
                      <SimpleTooltip content={isCopied ? 'Copied!' : 'Copy to Clipboard'}>
                        <button
                          type="button"
                          onClick={() => handleCopy(item.id, item.text)}
                          style={{
                            display: 'flex',
                            alignItems: 'center',
                            gap: '4px',
                            padding: '5px 10px',
                            borderRadius: '6px',
                            backgroundColor: isCopied ? 'rgba(52, 211, 153, 0.15)' : 'rgba(255, 255, 255, 0.04)',
                            border: `1px solid ${isCopied ? 'rgba(52, 211, 153, 0.3)' : 'rgba(255, 255, 255, 0.08)'}`,
                            color: isCopied ? '#34d399' : '#cac4d7',
                            fontSize: '11px',
                            fontWeight: 500,
                            cursor: 'pointer',
                            transition: 'all 0.15s ease',
                          }}
                        >
                          {isCopied ? <Check style={{ width: '12px', height: '12px' }} /> : <Copy style={{ width: '12px', height: '12px' }} />}
                          <span>{isCopied ? 'Copied' : 'Copy'}</span>
                        </button>
                      </SimpleTooltip>

                      <SimpleTooltip content="Delete permanently">
                        <button
                          type="button"
                          onClick={(e) => handleDelete(item.id, e)}
                          style={{
                            background: 'transparent',
                            border: 'none',
                            color: '#6e6a7d',
                            cursor: 'pointer',
                            padding: '4px',
                            display: 'flex',
                            alignItems: 'center',
                            transition: 'color 0.15s ease',
                          }}
                          onMouseEnter={(e) => (e.currentTarget.style.color = '#f87171')}
                          onMouseLeave={(e) => (e.currentTarget.style.color = '#6e6a7d')}
                        >
                          <Trash2 style={{ width: '14px', height: '14px' }} />
                        </button>
                      </SimpleTooltip>
                    </div>
                  </div>

                  {/* Transcribed Text body */}
                  <p
                    style={{
                      margin: 0,
                      fontSize: '13.5px',
                      lineHeight: 1.55,
                      color: '#f0eff4',
                      userSelect: 'text',
                      whiteSpace: 'pre-wrap',
                      wordBreak: 'break-word',
                    }}
                  >
                    {item.text}
                  </p>
                </SpotlightCard>
              );
            })}
          </div>
        )}

        {/* Clear History Confirmation Dialog */}
        <Dialog open={showClearConfirmDialog} onOpenChange={setShowClearConfirmDialog}>
          <DialogContent>
            <DialogHeader>
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px', color: '#f87171' }}>
                <AlertTriangle style={{ width: '18px', height: '18px' }} />
                <DialogTitle>Clear All History?</DialogTitle>
              </div>
              <DialogDescription>
                This action cannot be undone. All {history.length} dictation entries stored in your local SQLite database will be permanently removed.
              </DialogDescription>
            </DialogHeader>
            <DialogFooter>
              <button
                type="button"
                onClick={() => setShowClearConfirmDialog(false)}
                style={{
                  padding: '7px 14px',
                  borderRadius: '7px',
                  backgroundColor: '#1a1a24',
                  border: '1px solid rgba(255, 255, 255, 0.08)',
                  color: '#cac4d7',
                  fontSize: '12px',
                  cursor: 'pointer',
                }}
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={handleConfirmClear}
                style={{
                  padding: '7px 14px',
                  borderRadius: '7px',
                  backgroundColor: '#dc2626',
                  border: 'none',
                  color: '#ffffff',
                  fontSize: '12px',
                  fontWeight: 600,
                  cursor: 'pointer',
                }}
              >
                Permanently Delete All
              </button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      </div>
    </div>
  );
};
