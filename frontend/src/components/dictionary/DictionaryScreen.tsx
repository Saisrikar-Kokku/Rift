import React, { useState, useEffect, useMemo, useCallback } from 'react';
import { invokeTauri } from '../../lib/tauriBridge';
import { SpotlightCard } from '../ui/SpotlightCard';
import { Badge } from '../ui/Badge';
import { Switch } from '../ui/Switch';
import { SimpleTooltip } from '../ui/Tooltip';
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from '../ui/Dialog';
import { Plus, Search, Trash2, BookOpen, Sparkles, ArrowRight, Download, Upload, Check } from 'lucide-react';

interface DictionaryEntry {
  id: string;
  phrase: string;
  replacement: string;
  enabled: boolean;
  tag?: string;
}

export const DictionaryScreen: React.FC = () => {
  const [rules, setRules] = useState<DictionaryEntry[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [activeCategory, setActiveCategory] = useState<string>('all');
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [newPhrase, setNewPhrase] = useState('');
  const [newReplacement, setNewReplacement] = useState('');
  const [newTag, setNewTag] = useState('Tech & Code');

  // Interactive Live Transformation Playground state
  const [testInput, setTestInput] = useState('');

  const loadDictionary = useCallback(async () => {
    try {
      const dict = await invokeTauri<DictionaryEntry[]>('getDictionary');
      if (Array.isArray(dict)) {
        setRules(dict);
      }
    } catch (e) {
      console.warn('Failed to load dictionary from backend', e);
    }
  }, []);

  useEffect(() => {
    loadDictionary();
  }, [loadDictionary]);

  const handleToggleRule = async (id: string, currentEnabled: boolean) => {
    const nextEnabled = !currentEnabled;
    setRules((prev) =>
      prev.map((r) => (r.id === id ? { ...r, enabled: nextEnabled } : r))
    );
    try {
      await invokeTauri('updateDictionaryEntry', { id, patch: { enabled: nextEnabled } });
    } catch (err) {
      console.error('Failed to update dictionary rule', err);
      loadDictionary();
    }
  };

  const handleDeleteRule = async (id: string) => {
    setRules((prev) => prev.filter((r) => r.id !== id));
    try {
      await invokeTauri('deleteDictionaryEntry', { id });
    } catch (err) {
      console.error('Failed to delete dictionary rule', err);
      loadDictionary();
    }
  };

  const handleAddRule = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newPhrase.trim() || !newReplacement.trim()) return;

    const entryPayload = {
      phrase: newPhrase.trim(),
      replacement: newReplacement.trim(),
      tag: newTag,
    };

    try {
      const created = await invokeTauri<DictionaryEntry>('addDictionaryEntry', { entry: entryPayload });
      if (created && created.id) {
        setRules((prev) => [created, ...prev]);
      } else {
        loadDictionary();
      }
    } catch (err) {
      console.error('Failed to add dictionary entry', err);
      loadDictionary();
    }

    setNewPhrase('');
    setNewReplacement('');
    setIsModalOpen(false);
  };

  // Compute live transformed text for the Playground
  const transformedText = useMemo(() => {
    if (!testInput) return '';
    let result = testInput;
    const activeRules = rules.filter((r) => r.enabled);
    for (const rule of activeRules) {
      if (rule.phrase && rule.replacement) {
        const regex = new RegExp(`\\b${rule.phrase.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\b`, 'gi');
        result = result.replace(regex, rule.replacement);
      }
    }
    return result;
  }, [testInput, rules]);

  // Categories
  const categories = useMemo(() => {
    const set = new Set<string>();
    rules.forEach((r) => {
      if (r.tag) set.add(r.tag);
    });
    return ['all', ...Array.from(set)];
  }, [rules]);

  const filteredRules = useMemo(() => {
    const q = searchQuery.toLowerCase().trim();
    return rules.filter((r) => {
      if (q && !r.phrase.toLowerCase().includes(q) && !r.replacement.toLowerCase().includes(q)) {
        return false;
      }
      if (activeCategory !== 'all' && r.tag !== activeCategory) {
        return false;
      }
      return true;
    });
  }, [rules, searchQuery, activeCategory]);

  return (
    <div className="rift-main-area">
      <div style={{ maxWidth: '1040px', margin: '0 auto', display: 'flex', flexDirection: 'column', gap: '20px' }}>
        
        {/* Header row */}
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', flexWrap: 'wrap', gap: '14px' }}>
          <div>
            <h1 style={{ margin: 0, fontSize: '24px', fontWeight: 700, color: '#f0eff4', letterSpacing: '-0.02em' }}>
              Custom Dictionary
            </h1>
            <p style={{ margin: '4px 0 0', fontSize: '13px', color: '#9c97aa' }}>
              Enforce phonetic word replacements, jargon, and proper nouns into your speech pipeline.
            </p>
          </div>

          <button
            type="button"
            onClick={() => setIsModalOpen(true)}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: '6px',
              padding: '8px 16px',
              borderRadius: '8px',
              backgroundColor: 'var(--accent-purple)',
              color: '#0c0c12',
              fontSize: '12.5px',
              fontWeight: 600,
              border: 'none',
              cursor: 'pointer',
              transition: 'opacity 0.15s ease',
            }}
          >
            <Plus style={{ width: '15px', height: '15px' }} />
            Add Custom Rule
          </button>
        </div>

        {/* ================= DUAL PANE: RULES LIST + LIVE TRANSFORMATION PLAYGROUND ================= */}
        <div style={{ display: 'grid', gridTemplateColumns: 'minmax(0, 1.4fr) minmax(0, 1fr)', gap: '18px', alignItems: 'start' }}>
          
          {/* LEFT PANE: SEARCH, FILTER, AND RULES LIST */}
          <div style={{ display: 'flex', flexDirection: 'column', gap: '14px' }}>
            
            {/* Search and Category Filter Row */}
            <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
              <div style={{ position: 'relative', flex: 1 }}>
                <Search
                  style={{
                    position: 'absolute',
                    left: '12px',
                    top: '50%',
                    transform: 'translateY(-50%)',
                    width: '14px',
                    height: '14px',
                    color: '#6e6a7d',
                  }}
                />
                <input
                  type="text"
                  placeholder="Filter dictionary terms..."
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  style={{
                    width: '100%',
                    padding: '8px 12px 8px 34px',
                    borderRadius: '8px',
                    backgroundColor: '#13131c',
                    border: '1px solid rgba(255, 255, 255, 0.08)',
                    color: '#f0eff4',
                    fontSize: '12px',
                    outline: 'none',
                    boxSizing: 'border-box',
                  }}
                />
              </div>

              {/* Tag selector */}
              <select
                value={activeCategory}
                onChange={(e) => setActiveCategory(e.target.value)}
                style={{
                  padding: '8px 12px',
                  borderRadius: '8px',
                  backgroundColor: '#13131c',
                  border: '1px solid rgba(255, 255, 255, 0.08)',
                  color: '#cac4d7',
                  fontSize: '12px',
                  outline: 'none',
                  cursor: 'pointer',
                }}
              >
                {categories.map((c) => (
                  <option key={c} value={c}>
                    {c === 'all' ? 'All Tags' : c}
                  </option>
                ))}
              </select>
            </div>

            {/* Rules list */}
            {filteredRules.length === 0 ? (
              <SpotlightCard style={{ padding: '40px 20px', textAlign: 'center' }}>
                <BookOpen style={{ width: '32px', height: '32px', color: '#6e6a7d', margin: '0 auto 10px' }} />
                <span style={{ fontSize: '13px', color: '#9c97aa', display: 'block' }}>
                  No dictionary rules found. Click "Add Custom Rule" to create your first word replacement.
                </span>
              </SpotlightCard>
            ) : (
              <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
                {filteredRules.map((rule) => (
                  <SpotlightCard
                    key={rule.id}
                    spotlightColor="rgba(167, 139, 250, 0.06)"
                    style={{
                      padding: '12px 16px',
                      display: 'flex',
                      alignItems: 'center',
                      justifyContent: 'space-between',
                      opacity: rule.enabled ? 1 : 0.55,
                    }}
                  >
                    <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
                      <Switch
                        checked={rule.enabled}
                        onCheckedChange={() => handleToggleRule(rule.id, rule.enabled)}
                      />
                      <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
                        <span style={{ fontSize: '13px', fontWeight: 600, color: '#f0eff4', fontFamily: 'monospace' }}>
                          "{rule.phrase}"
                        </span>
                        <ArrowRight style={{ width: '13px', height: '13px', color: '#6e6a7d' }} />
                        <span style={{ fontSize: '13px', fontWeight: 600, color: 'var(--accent-purple)', fontFamily: 'monospace' }}>
                          "{rule.replacement}"
                        </span>
                        {rule.tag && <Badge variant="subtle" size="sm">{rule.tag}</Badge>}
                      </div>
                    </div>

                    <SimpleTooltip content="Delete rule">
                      <button
                        type="button"
                        onClick={() => handleDeleteRule(rule.id)}
                        style={{
                          background: 'transparent',
                          border: 'none',
                          color: '#6e6a7d',
                          cursor: 'pointer',
                          padding: '4px',
                        }}
                        onMouseEnter={(e) => (e.currentTarget.style.color = '#f87171')}
                        onMouseLeave={(e) => (e.currentTarget.style.color = '#6e6a7d')}
                      >
                        <Trash2 style={{ width: '14px', height: '14px' }} />
                      </button>
                    </SimpleTooltip>
                  </SpotlightCard>
                ))}
              </div>
            )}
          </div>

          {/* RIGHT PANE: INTERACTIVE LIVE TRANSFORMATION PLAYGROUND */}
          <SpotlightCard
            spotlightColor="rgba(52, 211, 153, 0.08)"
            style={{
              padding: '20px',
              display: 'flex',
              flexDirection: 'column',
              gap: '14px',
              position: 'sticky',
              top: '20px',
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
              <Sparkles style={{ width: '16px', height: '16px', color: '#34d399' }} />
              <div>
                <h3 style={{ margin: 0, fontSize: '14.5px', fontWeight: 600, color: '#f0eff4' }}>
                  Live Rule Playground
                </h3>
                <span style={{ fontSize: '11px', color: '#9c97aa' }}>
                  Test your phonetic substitutions instantaneously
                </span>
              </div>
            </div>

            <div>
              <label style={{ fontSize: '11.5px', color: '#9c97aa', fontWeight: 500, marginBottom: '6px', display: 'block' }}>
                Test Input Speech / Text:
              </label>
              <textarea
                rows={3}
                placeholder="Type or paste sample text here..."
                value={testInput}
                onChange={(e) => setTestInput(e.target.value)}
                style={{
                  width: '100%',
                  padding: '10px 12px',
                  borderRadius: '8px',
                  backgroundColor: '#101018',
                  border: '1px solid rgba(255, 255, 255, 0.08)',
                  color: '#f0eff4',
                  fontSize: '12.5px',
                  outline: 'none',
                  resize: 'vertical',
                  boxSizing: 'border-box',
                }}
              />
            </div>

            <div style={{ borderTop: '1px solid rgba(255, 255, 255, 0.06)', paddingTop: '12px' }}>
              <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '6px' }}>
                <label style={{ fontSize: '11.5px', color: '#9c97aa', fontWeight: 500 }}>
                  Substituted Output:
                </label>
                {testInput && (
                  <Badge variant="success" size="sm">
                    {rules.filter((r) => r.enabled).length} Rules Active
                  </Badge>
                )}
              </div>
              <div
                style={{
                  padding: '12px',
                  borderRadius: '8px',
                  backgroundColor: '#101018',
                  border: '1px solid rgba(255, 255, 255, 0.05)',
                  minHeight: '64px',
                  color: transformedText ? '#f0eff4' : '#6e6a7d',
                  fontSize: '13px',
                  lineHeight: 1.5,
                  userSelect: 'text',
                }}
              >
                {transformedText || 'Transformed text preview will appear here in real-time.'}
              </div>
            </div>
          </SpotlightCard>
        </div>

        {/* Add Rule Dialog */}
        <Dialog open={isModalOpen} onOpenChange={setIsModalOpen}>
          <DialogContent>
            <DialogHeader>
              <DialogTitle>Add Custom Vocabulary Rule</DialogTitle>
              <DialogDescription>
                Define a spoken word or phrase to automatically convert into your preferred term.
              </DialogDescription>
            </DialogHeader>

            <form onSubmit={handleAddRule} style={{ display: 'flex', flexDirection: 'column', gap: '14px', marginTop: '8px' }}>
              <div>
                <label style={{ fontSize: '12px', color: '#cac4d7', display: 'block', marginBottom: '4px' }}>
                  Spoken Phrase (as heard by Whisper):
                </label>
                <input
                  type="text"
                  placeholder="e.g. sai srikar or cuber netes"
                  value={newPhrase}
                  onChange={(e) => setNewPhrase(e.target.value)}
                  style={{
                    width: '100%',
                    padding: '8px 12px',
                    borderRadius: '7px',
                    backgroundColor: '#101018',
                    border: '1px solid rgba(255, 255, 255, 0.1)',
                    color: '#f0eff4',
                    fontSize: '13px',
                    outline: 'none',
                    boxSizing: 'border-box',
                  }}
                  required
                />
              </div>

              <div>
                <label style={{ fontSize: '12px', color: '#cac4d7', display: 'block', marginBottom: '4px' }}>
                  Replacement Text (injected output):
                </label>
                <input
                  type="text"
                  placeholder="e.g. Sai Srikar or Kubernetes"
                  value={newReplacement}
                  onChange={(e) => setNewReplacement(e.target.value)}
                  style={{
                    width: '100%',
                    padding: '8px 12px',
                    borderRadius: '7px',
                    backgroundColor: '#101018',
                    border: '1px solid rgba(255, 255, 255, 0.1)',
                    color: '#f0eff4',
                    fontSize: '13px',
                    outline: 'none',
                    boxSizing: 'border-box',
                  }}
                  required
                />
              </div>

              <div>
                <label style={{ fontSize: '12px', color: '#cac4d7', display: 'block', marginBottom: '4px' }}>
                  Category Tag:
                </label>
                <select
                  value={newTag}
                  onChange={(e) => setNewTag(e.target.value)}
                  style={{
                    width: '100%',
                    padding: '8px 12px',
                    borderRadius: '7px',
                    backgroundColor: '#101018',
                    border: '1px solid rgba(255, 255, 255, 0.1)',
                    color: '#f0eff4',
                    fontSize: '13px',
                    outline: 'none',
                    boxSizing: 'border-box',
                  }}
                >
                  <option value="Tech & Code">Tech & Code</option>
                  <option value="Proper Names">Proper Names</option>
                  <option value="Telugu / Tenglish">Telugu / Tenglish</option>
                  <option value="Medical & Science">Medical & Science</option>
                  <option value="Acronyms">Acronyms</option>
                </select>
              </div>

              <DialogFooter style={{ marginTop: '8px' }}>
                <button
                  type="button"
                  onClick={() => setIsModalOpen(false)}
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
                  type="submit"
                  style={{
                    padding: '7px 14px',
                    borderRadius: '7px',
                    backgroundColor: 'var(--accent-purple)',
                    border: 'none',
                    color: '#0c0c12',
                    fontSize: '12px',
                    fontWeight: 600,
                    cursor: 'pointer',
                  }}
                >
                  Save Word Rule
                </button>
              </DialogFooter>
            </form>
          </DialogContent>
        </Dialog>
      </div>
    </div>
  );
};
