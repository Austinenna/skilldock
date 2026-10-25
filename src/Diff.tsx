import { useEffect, useState } from 'react';
import { api } from './ipc';
import type { Palette } from './palettes';
import type { FileChange } from './types';
import { MONO, Section } from './ui';

const MARKS: Record<FileChange['kind'], { mark: string; color: string; label: string }> = {
  added: { mark: '+', color: '#1f9d55', label: '新增' },
  removed: { mark: '−', color: '#d93025', label: '删除' },
  modified: { mark: 'M', color: '#b26a00', label: '修改' },
};

function Patch({ text, p }: { text: string; p: Palette }) {
  return (
    <pre style={{ margin: 0, padding: '8px 0', fontFamily: MONO, fontSize: 11, lineHeight: 1.55, overflowX: 'auto', background: 'rgba(0,0,0,0.025)', borderTop: '1px solid ' + p.lineSoft }}>
      {text.split('\n').map((line, i) => {
        const tone = line.startsWith('@@')
          ? { color: '#0b7285', bg: 'rgba(11,114,133,0.06)' }
          : line.startsWith('+')
            ? { color: '#1f6f3a', bg: 'rgba(31,157,85,0.10)' }
            : line.startsWith('-')
              ? { color: '#a1261b', bg: 'rgba(217,48,37,0.09)' }
              : { color: p.text2, bg: 'transparent' };
        return (
          <div key={i} style={{ padding: '0 12px', color: tone.color, background: tone.bg, whiteSpace: 'pre-wrap', overflowWrap: 'anywhere', minHeight: '1.55em' }}>{line}</div>
        );
      })}
    </pre>
  );
}

/** Unpublished changes: dev/<id> compared with released/<id>. Same rules as `skl diff`. */
export function DiffSection({ skillId, refreshKey, p }: { skillId: string; refreshKey: unknown; p: Palette }) {
  const [open, setOpen] = useState(true);
  const [changes, setChanges] = useState<FileChange[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [tick, setTick] = useState(0);

  useEffect(() => { setExpanded(new Set()); }, [skillId]);
  useEffect(() => {
    let alive = true;
    setError(null);
    api.getSkillDiff(skillId)
      .then((c) => {
        if (!alive) return;
        setChanges(c);
        // Small change sets open fully; large ones start collapsed.
        setExpanded((prev) => (prev.size || c.length > 3 ? prev : new Set(c.filter((x) => x.patch).map((x) => x.path))));
      })
      .catch((e) => alive && setError(String(e)));
    return () => { alive = false; };
  }, [skillId, refreshKey, tick]);

  const toggle = (path: string) => setExpanded((prev) => {
    const next = new Set(prev);
    if (next.has(path)) next.delete(path); else next.add(path);
    return next;
  });

  const refresh = (
    <button type="button" title="重新比较" onClick={(e) => { e.stopPropagation(); setTick((t) => t + 1); }}
      style={{ border: 0, background: 'transparent', color: p.text3, cursor: 'pointer', fontSize: 13, padding: 0 }}>↻</button>
  );

  return (
    <Section title="未发布改动" badge={changes ? `${changes.length} 个文件` : undefined} open={open} setOpen={setOpen} action={refresh} p={p}>
      <div style={{ padding: '4px 28px 20px', fontSize: 12 }}>
        {error ? (
          <div style={{ color: p.danger }}>{error}</div>
        ) : !changes ? (
          <div style={{ color: p.text3 }}>比较中…</div>
        ) : changes.length === 0 ? (
          <div style={{ color: p.text3 }}>开发区和发布版一致。</div>
        ) : (
          <div style={{ border: '1px solid ' + p.line, borderRadius: 8, overflow: 'hidden' }}>
            {changes.map((c, i) => {
              const m = MARKS[c.kind];
              const canOpen = !!c.patch;
              const isOpen = canOpen && expanded.has(c.path);
              const size = c.kind === 'added' ? c.newSize : c.kind === 'removed' ? c.oldSize : `${c.oldSize} → ${c.newSize}`;
              return (
                <div key={c.path} style={{ borderBottom: i < changes.length - 1 ? '1px solid ' + p.lineSoft : 'none' }}>
                  <div onClick={() => canOpen && toggle(c.path)}
                    style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '7px 10px', cursor: canOpen ? 'pointer' : 'default', fontFamily: MONO }}>
                    <span title={m.label} style={{ width: 14, textAlign: 'center', fontWeight: 700, color: m.color, flexShrink: 0 }}>{m.mark}</span>
                    <span title={c.path} style={{ flex: 1, minWidth: 0, color: p.text, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{c.path}</span>
                    {c.binary && <span style={{ fontSize: 10, color: p.text3 }}>二进制</span>}
                    <span style={{ fontSize: 11, color: p.text3, whiteSpace: 'nowrap' }}>{size}</span>
                    <span style={{ width: 10, fontSize: 9, color: p.text3, display: 'inline-block', transform: isOpen ? 'rotate(90deg)' : 'none', visibility: canOpen ? 'visible' : 'hidden' }}>▶</span>
                  </div>
                  {isOpen && c.patch && <Patch text={c.patch.replace(/\n$/, '')} p={p} />}
                </div>
              );
            })}
          </div>
        )}
      </div>
    </Section>
  );
}
