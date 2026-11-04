import React from 'react';
import { Check, Clipboard, FileText, StickyNote, Copy, CornerDownLeft } from 'lucide-react';

export type LibraryItemKind = 'transcript' | 'clipboard' | 'note';

export interface LibraryItem {
  id: string;
  kind: LibraryItemKind;
  timestamp: string;
  source_app?: string | null;
  category?: string | null;
  excerpt: string;
}

interface LibraryResultCardProps {
  item: LibraryItem;
  copied: boolean;
  onCopy: (item: LibraryItem) => void;
  onInsert: (item: LibraryItem) => void;
}

const kindDetails: Record<LibraryItemKind, { label: string; icon: React.ElementType; color: string }> = {
  transcript: { label: 'Transcript', icon: FileText, color: '#c4b5fd' },
  clipboard: { label: 'Clipboard', icon: Clipboard, color: '#93c5fd' },
  note: { label: 'Scratchpad', icon: StickyNote, color: '#34d399' },
};

function formatTimestamp(timestamp: string): string {
  const date = new Date(timestamp);
  if (Number.isNaN(date.getTime())) return timestamp;

  return new Intl.DateTimeFormat(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  }).format(date);
}

export const LibraryResultCard: React.FC<LibraryResultCardProps> = ({ item, copied, onCopy, onInsert }) => {
  const details = kindDetails[item.kind];
  const KindIcon = details.icon;

  return (
    <article
      style={{
        padding: '16px 18px',
        borderRadius: '10px',
        background: 'var(--bg-card, #13131c)',
        border: '1px solid var(--border-subtle, rgba(255,255,255,0.07))',
        display: 'flex',
        flexDirection: 'column',
        gap: '12px',
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: '12px' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px', minWidth: 0 }}>
          <KindIcon aria-hidden="true" style={{ width: '15px', height: '15px', color: details.color, flexShrink: 0 }} />
          <span style={{ color: details.color, fontSize: '12px', fontWeight: 700 }}>{details.label}</span>
          {item.category && item.kind === 'clipboard' && (
            <span
              style={{
                padding: '2px 7px',
                borderRadius: '999px',
                background: 'rgba(255,255,255,0.05)',
                color: 'var(--text-secondary, #9c97aa)',
                fontSize: '10px',
                textTransform: 'capitalize',
              }}
            >
              {item.category}
            </span>
          )}
          {item.source_app && (
            <span
              title={item.source_app}
              style={{
                color: 'var(--text-muted, #6e6a7d)',
                fontSize: '11px',
                overflow: 'hidden',
                textOverflow: 'ellipsis',
                whiteSpace: 'nowrap',
              }}
            >
              · {item.source_app}
            </span>
          )}
        </div>
        <time
          dateTime={item.timestamp}
          style={{ color: 'var(--text-muted, #6e6a7d)', fontSize: '11px', whiteSpace: 'nowrap' }}
        >
          {formatTimestamp(item.timestamp)}
        </time>
      </div>

      <p
        style={{
          margin: 0,
          color: 'var(--text-primary, #f0eff4)',
          fontSize: '13px',
          lineHeight: 1.6,
          whiteSpace: 'pre-wrap',
          overflowWrap: 'anywhere',
        }}
      >
        {item.excerpt}
      </p>

      <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '8px' }}>
        <button
          type="button"
          onClick={() => onCopy(item)}
          aria-label={copied ? 'Copied to clipboard' : 'Copy full item to clipboard'}
          style={actionButtonStyle}
        >
          {copied ? <Check aria-hidden="true" size={14} /> : <Copy aria-hidden="true" size={14} />}
          {copied ? 'Copied' : 'Copy'}
        </button>
        <button
          type="button"
          onClick={() => onInsert(item)}
          aria-label="Insert full item into the last active application"
          style={{ ...actionButtonStyle, color: '#c4b5fd', borderColor: 'rgba(196,181,253,0.2)' }}
        >
          <CornerDownLeft aria-hidden="true" size={14} />
          Insert
        </button>
      </div>
    </article>
  );
};

const actionButtonStyle: React.CSSProperties = {
  minHeight: '34px',
  display: 'inline-flex',
  alignItems: 'center',
  justifyContent: 'center',
  gap: '6px',
  padding: '0 10px',
  borderRadius: '7px',
  background: 'rgba(255,255,255,0.03)',
  border: '1px solid rgba(255,255,255,0.08)',
  color: 'var(--text-secondary, #9c97aa)',
  fontSize: '11px',
  fontWeight: 600,
  cursor: 'pointer',
};
