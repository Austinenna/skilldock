import { useState, type ReactNode } from 'react';
import type { Palette } from './palettes';
import type { Agent } from './types';
import { AGENT_ACCENTS, DragSpacer, agentName } from './ui';

/** 'all' | 'state:<SkillState>' | 'agent:<id>' | 'doctor' | 'settings' */
export type ViewKey = string;

function SidebarSection({ title, p, children }: { title: string; p: Palette; children: ReactNode }) {
  return (
    <div style={{ padding: '14px 0 4px' }}>
      <div style={{ padding: '0 16px 6px', fontSize: 11, fontWeight: 700, color: p.text3, letterSpacing: 0.2, textTransform: 'uppercase' }}>{title}</div>
      {children}
    </div>
  );
}

function SidebarItem({ icon, label, count, active, accent, dim, onClick, p }: {
  icon: string; label: string; count?: number; active: boolean; accent?: string; dim?: boolean; onClick: () => void; p: Palette;
}) {
  const [hover, setHover] = useState(false);
  return (
    <div onClick={onClick} onMouseEnter={() => setHover(true)} onMouseLeave={() => setHover(false)}
      style={{
        margin: '0 8px', padding: '5px 8px', borderRadius: 6,
        display: 'flex', alignItems: 'center', gap: 8,
        fontSize: 13, color: active ? '#fff' : p.text, opacity: dim && !active ? 0.55 : 1,
        backgroundColor: active ? p.accent : (hover ? 'rgba(0,0,0,0.04)' : 'transparent'),
        cursor: 'pointer', userSelect: 'none', height: 24,
      }}>
      <div style={{
        width: 16, height: 16, borderRadius: 4,
        background: active ? 'rgba(255,255,255,0.22)' : (accent || 'rgba(0,0,0,0.12)'),
        flexShrink: 0, display: 'grid', placeItems: 'center',
        fontSize: 10, color: active ? '#fff' : 'rgba(0,0,0,0.55)', fontWeight: 700,
      }}>{icon}</div>
      <span style={{ flex: 1, fontWeight: active ? 500 : 400, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{label}</span>
      {count != null && (
        <span style={{ fontSize: 11, color: active ? 'rgba(255,255,255,0.85)' : p.text3, fontVariantNumeric: 'tabular-nums' }}>{count}</span>
      )}
    </div>
  );
}

export function Sidebar({ active, setActive, counts, agents, issueCount, root, p }: {
  active: ViewKey; setActive: (s: ViewKey) => void; counts: Record<string, number>;
  agents: Agent[]; issueCount: number; root: string; p: Palette;
}) {
  const item = (key: ViewKey, icon: string, label: string, accent?: string, dim?: boolean) => (
    <SidebarItem key={key} icon={icon} label={label} count={counts[key]} accent={accent} dim={dim}
      active={active === key} onClick={() => setActive(key)} p={p} />
  );
  return (
    <div style={{ width: 200, background: p.sidebar, borderRight: '1px solid ' + p.line, display: 'flex', flexDirection: 'column', flexShrink: 0 }}>
      <DragSpacer />
      <div style={{ flex: 1, overflowY: 'auto', paddingBottom: 12 }}>
        <SidebarSection title="Library" p={p}>
          {item('all', '◎', '全部', '#34c759')}
          {item('state:devOnly', '✎', '开发中', 'rgba(0,0,0,0.12)')}
          {item('state:devAhead', '↑', '有未发布改动', '#ff9f0a')}
          {item('state:released', '✓', '已发布', '#34c759')}
        </SidebarSection>
        {agents.length > 0 && (
          <SidebarSection title="Agents" p={p}>
            {agents.map((a) => item('agent:' + a.id, agentName(a.id)[0], agentName(a.id), AGENT_ACCENTS[a.id], !a.installed))}
          </SidebarSection>
        )}
        <SidebarSection title="Tools" p={p}>
          <SidebarItem icon="✚" label="Doctor" count={issueCount || undefined} accent={issueCount ? '#ff3b30' : '#8e8e93'}
            active={active === 'doctor'} onClick={() => setActive('doctor')} p={p} />
          <SidebarItem icon="⚙" label="Settings" accent="#8e8e93"
            active={active === 'settings'} onClick={() => setActive('settings')} p={p} />
        </SidebarSection>
      </div>
      <div title={root} style={{ padding: '8px 12px', fontSize: 11, color: p.text3, borderTop: '1px solid ' + p.lineSoft, display: 'flex', alignItems: 'center', gap: 6, overflow: 'hidden', whiteSpace: 'nowrap', textOverflow: 'ellipsis' }}>
        <span style={{ width: 6, height: 6, borderRadius: 3, background: p.hub, flexShrink: 0 }} />
        {counts.all ?? 0} skills · {root}
      </div>
    </div>
  );
}
