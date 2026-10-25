import { useState } from 'react';
import type { Palette } from './palettes';
import type { Skill } from './types';
import { AgentChips, FONT_SANS, MONO, StateBadge } from './ui';

export function ListHeader({ title, q, setQ, view, setView, onNew, onRefresh, refreshing, p, density }: {
  title: string; q: string; setQ: (s: string) => void; view: 'list' | 'grid';
  setView: (v: 'list' | 'grid') => void; onNew: () => void; onRefresh: () => void;
  refreshing: boolean; p: Palette; density: string;
}) {
  return (
    <div style={{ borderBottom: '1px solid ' + p.line, padding: density === 'compact' ? '8px 12px' : '10px 12px 10px 14px', display: 'flex', flexDirection: 'column', gap: 8, background: p.panel }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <div style={{ fontSize: 13, fontWeight: 600, color: p.text, minWidth: 0, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{title}</div>
        <div style={{ flex: 1 }} />
        <button onClick={onRefresh} disabled={refreshing} aria-label="Refresh skills" title="Refresh" style={{
          width: 28, height: 28, border: '1px solid ' + p.line, background: p.panel,
          color: p.text, cursor: refreshing ? 'not-allowed' : 'pointer',
          borderRadius: 6, padding: 0, fontSize: 15, fontWeight: 600, fontFamily: FONT_SANS,
          opacity: refreshing ? 0.75 : 1, display: 'grid', placeItems: 'center', flexShrink: 0,
        }}>
          <span style={{ display: 'inline-block', lineHeight: 1, animation: refreshing ? 'skilldock-spin 0.8s linear infinite' : undefined }}>↻</span>
        </button>
        <div style={{ display: 'flex', background: 'rgba(0,0,0,0.05)', borderRadius: 6, padding: 2 }}>
          {(['list', 'grid'] as const).map((m) => (
            <button key={m} onClick={() => setView(m)}
              style={{
                border: 0, background: view === m ? p.panel : 'transparent', cursor: 'pointer',
                borderRadius: 4, padding: '3px 8px', fontSize: 11, color: p.text, fontFamily: FONT_SANS,
                boxShadow: view === m ? '0 1px 2px rgba(0,0,0,0.08)' : 'none',
              }}>{m === 'list' ? '☰' : '▦'}</button>
          ))}
        </div>
        <button onClick={onNew} style={{
          border: 0, background: p.accent, color: '#fff', cursor: 'pointer',
          borderRadius: 6, padding: '4px 10px', fontSize: 12, fontWeight: 500, fontFamily: FONT_SANS,
          display: 'flex', alignItems: 'center', gap: 4, whiteSpace: 'nowrap', flexShrink: 0, minHeight: 28,
        }}>＋ New</button>
      </div>
      <div style={{ position: 'relative' }}>
        <span style={{ position: 'absolute', left: 8, top: '50%', transform: 'translateY(-50%)', color: p.text3, fontSize: 12 }}>⌕</span>
        <input value={q} onChange={(e) => setQ(e.target.value)} placeholder="Filter skills…"
          style={{
            width: '100%', boxSizing: 'border-box', border: 'none', background: 'rgba(0,0,0,0.05)',
            borderRadius: 6, padding: '5px 8px 5px 24px', fontSize: 12, color: p.text, outline: 'none', fontFamily: FONT_SANS,
          }} />
      </div>
    </div>
  );
}

export function SkillRow({ skill, selected, onClick, p, density }: {
  skill: Skill; selected: boolean; onClick: () => void; p: Palette; density: string;
}) {
  const [hover, setHover] = useState(false);
  return (
    <div onClick={onClick} onMouseEnter={() => setHover(true)} onMouseLeave={() => setHover(false)}
      style={{
        padding: density === 'compact' ? '7px 14px' : '10px 14px', borderBottom: '1px solid ' + p.lineSoft,
        backgroundColor: selected ? p.rowSel : (hover ? p.rowHover : 'transparent'),
        cursor: 'pointer', display: 'flex', gap: 10, alignItems: 'flex-start',
      }}>
      <div style={{
        width: 28, height: 28, borderRadius: 6, background: p.iconBg,
        color: p.text2, fontSize: 13, fontWeight: 700, fontFamily: MONO,
        display: 'grid', placeItems: 'center', flexShrink: 0, marginTop: 2,
      }}>{skill.id[0]}</div>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
          <span style={{ fontSize: 13, fontWeight: 500, color: p.text, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{skill.id}</span>
          <StateBadge state={skill.state} p={p} />
        </div>
        <div style={{ fontSize: 12, color: p.text2, marginTop: 2, lineHeight: 1.4, overflow: 'hidden', textOverflow: 'ellipsis', display: '-webkit-box', WebkitLineClamp: 1, WebkitBoxOrient: 'vertical' }}>
          {skill.description || <span style={{ color: p.text3, fontStyle: 'italic' }}>No description</span>}
        </div>
        {density !== 'compact' && skill.agents.length > 0 && (
          <div style={{ display: 'flex', gap: 3, marginTop: 6, flexWrap: 'wrap' }}><AgentChips agents={skill.agents} p={p} /></div>
        )}
      </div>
    </div>
  );
}

export function SkillCard({ skill, selected, onClick, p }: {
  skill: Skill; selected: boolean; onClick: () => void; p: Palette;
}) {
  return (
    <div onClick={onClick}
      style={{
        padding: 12, background: selected ? p.rowSel : p.panel,
        border: '1px solid ' + (selected ? p.accent : p.line),
        borderRadius: 8, cursor: 'pointer', display: 'flex', flexDirection: 'column', gap: 6,
      }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <div style={{ width: 22, height: 22, borderRadius: 5, background: p.iconBg, color: p.text2, fontSize: 11, fontWeight: 700, fontFamily: MONO, display: 'grid', placeItems: 'center', flexShrink: 0 }}>{skill.id[0]}</div>
        <div style={{ fontSize: 12, fontWeight: 600, color: p.text, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap', flex: 1 }}>{skill.id}</div>
        <StateBadge state={skill.state} p={p} />
      </div>
      <div style={{ fontSize: 11, color: p.text2, lineHeight: 1.4, overflow: 'hidden', display: '-webkit-box', WebkitLineClamp: 2, WebkitBoxOrient: 'vertical' }}>
        {skill.description || <span style={{ color: p.text3, fontStyle: 'italic' }}>No description</span>}
      </div>
      <div style={{ display: 'flex', gap: 3, flexWrap: 'wrap', minHeight: 14 }}><AgentChips agents={skill.agents} p={p} /></div>
    </div>
  );
}
