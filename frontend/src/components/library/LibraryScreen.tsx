import React, { useCallback, useEffect, useRef, useState } from 'react';
import { ChevronLeft, ChevronRight, Search, X } from 'lucide-react';
import { invokeTauri, listenTauriEvent } from '../../lib/tauriBridge';
import { Kbd } from '../ui/Kbd';
import { LibraryResultCard } from './LibraryResultCard';
import type { LibraryItem, LibraryItemKind } from './LibraryResultCard';

const PAGE_SIZE = 30;
const SEARCH_TIMEOUT_MS = 10_000;
const sourceFilters: Array<{ id: 'all' | LibraryItemKind; label: string }> = [
  { id: 'all', label: 'Everything' },
  { id: 'transcript', label: 'Transcripts' },
  { id: 'clipboard', label: 'Clipboard' },
  { id: 'note', label: 'Scratchpad' },
];

interface LibrarySearchPage {
  results: LibraryItem[];
  total: number;
  limit: number;
  offset: number;
}

export const LibraryScreen: React.FC = () => {
  const searchInput = useRef<HTMLInputElement>(null);
  const [inputValue, setInputValue] = useState('');
  const [query, setQuery] = useState('');
  const [kind, setKind] = useState<'all' | LibraryItemKind>('all');
  const [page, setPage] = useState(0);
  const [searchResults, setSearchResults] = useState<LibrarySearchPage | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [refreshToken, setRefreshToken] = useState(0);
  const refreshTimer = useRef<number | null>(null);

  useEffect(() => {
    const timer = window.setTimeout(() => {
      setQuery(inputValue.trim());
      setPage(0);
    }, 220);
    return () => window.clearTimeout(timer);
  }, [inputValue]);

  useEffect(() => {
    searchInput.current?.focus();
    const focusSearch = () => searchInput.current?.focus();
    window.addEventListener('rift:focus-library-search', focusSearch);
    return () => window.removeEventListener('rift:focus-library-search', focusSearch);
  }, []);

  useEffect(() => {
    let cancelled = false;
    let legacyDraft: string | null = null;
    try {
      legacyDraft = window.localStorage.getItem('rift_scratchpad_draft');
    } catch {
      return;
    }
    if (!legacyDraft?.trim()) return;

    invokeTauri<{ content: string; updated_at: string | null }>('getScratchpadDraft')
      .then((draft) => {
        if (!cancelled && !draft.updated_at) {
          return invokeTauri('saveScratchpadDraft', { content: legacyDraft });
        }
      })
      .catch((reason) => console.warn('Could not migrate the existing Scratchpad draft:', reason));

    return () => { cancelled = true; };
  }, []);

  useEffect(() => {
    const refresh = () => {
      window.clearTimeout(refreshTimer.current ?? undefined);
      refreshTimer.current = window.setTimeout(() => {
        setRefreshToken((value) => value + 1);
      }, 350);
    };
    const unlisten = [
      listenTauriEvent('libraryUpdated', refresh),
      listenTauriEvent('clipboardChanged', refresh),
      listenTauriEvent('transcriptionSuccess', refresh),
    ];
    return () => {
      window.clearTimeout(refreshTimer.current ?? undefined);
      unlisten.forEach((stop) => stop());
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    let timedOut = false;
    setLoading(true);
    setError(null);

    const timeout = window.setTimeout(() => {
      if (!cancelled) {
        timedOut = true;
        setError('Library search took too long. Please retry.');
        setLoading(false);
      }
    }, SEARCH_TIMEOUT_MS);

    invokeTauri<LibrarySearchPage>('searchLibrary', {
      query,
      kind: kind === 'all' ? null : kind,
      limit: PAGE_SIZE,
      offset: page * PAGE_SIZE,
    })
      .then((data) => {
        if (timedOut) return;
        if (!data || !Array.isArray(data.results) || typeof data.total !== 'number') {
          throw new Error('Library returned an unexpected response. Please retry.');
        }
        if (!cancelled) setSearchResults(data);
      })
      .catch((reason) => {
        if (!cancelled && !timedOut) {
          setError(reason instanceof Error ? reason.message : 'Could not search your library.');
        }
      })
      .finally(() => {
        window.clearTimeout(timeout);
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
      window.clearTimeout(timeout);
    };
  }, [query, kind, page, refreshToken]);

  const handleCopy = useCallback(async (item: LibraryItem) => {
    try {
      const text = await invokeTauri<string | null>('getLibraryItemContent', { id: item.id });
      if (text === null) throw new Error('This library item is no longer available. Refresh and try again.');
      const copied = await invokeTauri<boolean>('copyToClipboard', { text });
      if (!copied && navigator.clipboard) await navigator.clipboard.writeText(text);
      setCopiedId(item.id);
      setNotice('Copied to clipboard');
      window.setTimeout(() => setCopiedId((current) => (current === item.id ? null : current)), 1600);
      window.setTimeout(() => setNotice(null), 2200);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : 'Could not copy this item.');
    }
  }, []);

  const handleInsert = useCallback(async (item: LibraryItem) => {
    try {
      const text = await invokeTauri<string | null>('getLibraryItemContent', { id: item.id });
      if (text === null) throw new Error('This library item is no longer available. Refresh and try again.');
      await invokeTauri('insertScratchpadText', { text });
      setNotice('Inserted into the last active application');
      window.setTimeout(() => setNotice(null), 2200);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : 'Could not insert this item.');
    }
  }, []);

  const results = searchResults?.results ?? [];
  const total = searchResults?.total ?? 0;
  const pageCount = Math.max(1, Math.ceil(total / PAGE_SIZE));
  const firstResult = total === 0 ? 0 : page * PAGE_SIZE + 1;
  const lastResult = Math.min((page + 1) * PAGE_SIZE, total);

  return (
    <main className="rift-main-area" aria-labelledby="library-title">
      <div style={{ maxWidth: '1040px', margin: '0 auto', display: 'flex', flexDirection: 'column', gap: '18px' }}>
        <header style={{ display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between', gap: '16px' }}>
          <div>
            <h1 id="library-title" style={{ margin: 0, fontSize: '24px', fontWeight: 700, letterSpacing: '-0.02em' }}>
              Library
            </h1>
            <p style={{ margin: '5px 0 0', color: 'var(--text-secondary, #9c97aa)', fontSize: '13px' }}>
              Search transcripts, clipboard history, and Scratchpad notes in one place.
            </p>
          </div>
          <div style={{ color: 'var(--text-muted, #6e6a7d)', display: 'flex', alignItems: 'center', gap: '6px', fontSize: '11px' }}>
            <span>Quick search</span><Kbd>Ctrl / ⌘ K</Kbd>
          </div>
        </header>

        <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
          <label htmlFor="library-search" style={{ position: 'absolute', width: 1, height: 1, padding: 0, margin: -1, overflow: 'hidden', clip: 'rect(0, 0, 0, 0)', whiteSpace: 'nowrap', border: 0 }}>
            Search transcripts, clipboard items, and Scratchpad notes
          </label>
          <div style={{ position: 'relative' }}>
            <Search aria-hidden="true" style={{ position: 'absolute', top: '50%', left: '14px', transform: 'translateY(-50%)', width: '17px', height: '17px', color: '#8d879a' }} />
            <input
              ref={searchInput}
              id="library-search"
              type="search"
              value={inputValue}
              onChange={(event) => setInputValue(event.target.value)}
              placeholder="Search text across your Rift activity…"
              autoComplete="off"
              spellCheck={false}
              style={{
                width: '100%',
                height: '46px',
                padding: '0 44px 0 42px',
                boxSizing: 'border-box',
                borderRadius: '10px',
                background: 'var(--bg-input, #101018)',
                border: '1px solid var(--border-subtle, rgba(255,255,255,0.08))',
                color: 'var(--text-primary, #f0eff4)',
                fontSize: '13px',
                outline: 'none',
              }}
            />
            {inputValue && (
              <button
                type="button"
                aria-label="Clear search"
                onClick={() => setInputValue('')}
                style={{ position: 'absolute', right: '10px', top: '50%', transform: 'translateY(-50%)', width: '28px', height: '28px', border: 0, borderRadius: '6px', background: 'transparent', color: '#9c97aa', cursor: 'pointer' }}
              >
                <X aria-hidden="true" size={15} />
              </button>
            )}
          </div>

          <div role="group" aria-label="Filter library by source" style={{ display: 'flex', flexWrap: 'wrap', gap: '7px' }}>
            {sourceFilters.map((filter) => {
              const active = kind === filter.id;
              return (
                <button
                  key={filter.id}
                  type="button"
                  aria-pressed={active}
                  onClick={() => { setKind(filter.id); setPage(0); }}
                  style={{
                    minHeight: '32px',
                    padding: '0 11px',
                    borderRadius: '7px',
                    border: `1px solid ${active ? 'rgba(196,181,253,0.25)' : 'rgba(255,255,255,0.07)'}`,
                    background: active ? 'rgba(196,181,253,0.11)' : 'rgba(255,255,255,0.02)',
                    color: active ? '#d4c8ff' : 'var(--text-secondary, #9c97aa)',
                    fontSize: '11px',
                    fontWeight: 600,
                    cursor: 'pointer',
                  }}
                >
                  {filter.label}
                </button>
              );
            })}
          </div>
        </div>

        <div aria-live="polite" aria-atomic="true" style={{ minHeight: '18px', display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: '12px' }}>
          <span style={{ color: 'var(--text-muted, #6e6a7d)', fontSize: '11px' }}>
            {loading ? 'Searching…' : total > 0 ? `Showing ${firstResult}–${lastResult} of ${total}` : '0 results'}
          </span>
          {notice && <span style={{ color: '#9ce8c8', fontSize: '11px' }}>{notice}</span>}
        </div>

        {error ? (
          <div role="alert" style={{ padding: '24px', textAlign: 'center', borderRadius: '10px', border: '1px solid rgba(248,113,113,0.2)', color: '#fca5a5', background: 'rgba(248,113,113,0.05)' }}>
            {error}
            <button type="button" onClick={() => setRefreshToken((value) => value + 1)} style={{ ...secondaryButtonStyle, marginLeft: '12px' }}>
              Try again
            </button>
          </div>
        ) : loading ? (
          <div aria-label="Loading library results" aria-busy="true" style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
            {[0, 1, 2].map((item) => <div key={item} style={{ height: '118px', borderRadius: '10px', background: 'rgba(255,255,255,0.025)', border: '1px solid rgba(255,255,255,0.04)' }} />)}
          </div>
        ) : results.length === 0 ? (
          <div role="status" style={{ padding: '54px 24px', textAlign: 'center', borderRadius: '10px', border: '1px solid var(--border-subtle, rgba(255,255,255,0.06))', background: 'rgba(255,255,255,0.015)' }}>
            <Search aria-hidden="true" size={22} style={{ color: '#797487', marginBottom: '10px' }} />
            <h2 style={{ margin: '0 0 5px', fontSize: '14px', fontWeight: 650 }}>
              {query ? 'No matching items' : 'Your library is ready'}
            </h2>
            <p style={{ margin: 0, color: 'var(--text-secondary, #9c97aa)', fontSize: '12px' }}>
              {query ? 'Try another phrase or choose a different source.' : 'New dictations, copied text, and Scratchpad notes will appear here.'}
            </p>
          </div>
        ) : (
          <>
            <div style={{ display: 'flex', flexDirection: 'column', gap: '9px' }}>
              {results.map((item) => (
                <LibraryResultCard
                  key={item.id}
                  item={item}
                  copied={copiedId === item.id}
                  onCopy={handleCopy}
                  onInsert={handleInsert}
                />
              ))}
            </div>

            <nav aria-label="Library result pages" style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: '12px', paddingTop: '4px' }}>
              <span style={{ color: 'var(--text-muted, #6e6a7d)', fontSize: '11px' }}>
                Page {page + 1} of {pageCount}
              </span>
              <div style={{ display: 'flex', gap: '7px' }}>
                <button type="button" disabled={page === 0} onClick={() => setPage((value) => Math.max(0, value - 1))} style={{ ...secondaryButtonStyle, opacity: page === 0 ? 0.45 : 1, cursor: page === 0 ? 'default' : 'pointer' }}>
                  <ChevronLeft aria-hidden="true" size={14} /> Previous
                </button>
                <button type="button" disabled={page + 1 >= pageCount} onClick={() => setPage((value) => value + 1)} style={{ ...secondaryButtonStyle, opacity: page + 1 >= pageCount ? 0.45 : 1, cursor: page + 1 >= pageCount ? 'default' : 'pointer' }}>
                  Next <ChevronRight aria-hidden="true" size={14} />
                </button>
              </div>
            </nav>
          </>
        )}
      </div>
    </main>
  );
};

const secondaryButtonStyle: React.CSSProperties = {
  minHeight: '32px',
  display: 'inline-flex',
  alignItems: 'center',
  justifyContent: 'center',
  gap: '5px',
  padding: '0 10px',
  borderRadius: '7px',
  border: '1px solid rgba(255,255,255,0.08)',
  background: 'rgba(255,255,255,0.03)',
  color: 'var(--text-secondary, #9c97aa)',
  fontSize: '11px',
  fontWeight: 600,
  cursor: 'pointer',
};
