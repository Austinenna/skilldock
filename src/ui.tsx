import { useEffect, useState, type MouseEvent, type ReactNode } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type { Palette } from './palettes';
import type { SkillState, SyncResult } from './types';

export const FONT_SANS = '-apple-system, BlinkMacSystemFont, "SF Pro Text", "Helvetica Neue", "Inter", sans-serif';
export const FONT_DISPLAY_WARM = '"Instrument Serif", Georgia, serif';
export const MONO = '"JetBrains Mono", "SF Mono", Menlo, monospace';

export const AGENT_ACCENTS: Record<string, string> = {
  claude: '#bf5af2', codex: '#5e9eff', openclaw: '#ff9f0a', workbuddy: '#10b981', kairos: '#ff375f',
};
export const AGENT_NAMES: Record<string, string> = {
  claude: 'Claude Code', codex: 'Codex', openclaw: 'OpenClaw', workbuddy: 'WorkBuddy', kairos: 'Kairos',
};
export const agentName = (id: string) => AGENT_NAMES[id] ?? id;

export const STATE_LABELS: Record<SkillState, string> = {
  devOnly: '开发中',
  released: '已发布',
  devAhead: '有未发布改动',
  releasedOnly: '无开发源',
  manifestOnly: '未发布',
};

export type NoticeTone = 'error' | 'info';
export interface AppNotice {
  id: number;
  tone: NoticeTone;
  message: string;
}

export function useFontLink() {
  useEffect(() => {
    if (!document.getElementById('skilldock-fonts')) {
      const l = document.createElement('link');
      l.id = 'skilldock-fonts';
      l.rel = 'stylesheet';
      l.href = 'https://fonts.googleapis.com/css2?family=Instrument+Serif:ital@0;1&family=JetBrains+Mono:wght@400;500&display=swap';
      document.head.appendChild(l);
    }
    if (!document.getElementById('skilldock-global-styles')) {
      const style = document.createElement('style');
      style.id = 'skilldock-global-styles';
      style.textContent = '@keyframes skilldock-spin { from { transform: rotate(0deg); } to { transform: rotate(360deg); } }';
      document.head.appendChild(style);
    }
  }, []);
}

function startWindowDrag(event: MouseEvent<HTMLElement>) {
  if (event.button !== 0) return;
  try {
    getCurrentWindow().startDragging().catch((error) => {
      console.error('Failed to start window drag', error);
    });
  } catch (error) {
    console.error('Failed to start window drag', error);
  }
}

export function WindowDragStrip() {
  return (
    <div data-tauri-drag-region onMouseDown={startWindowDrag}
      style={{ position: 'absolute', top: 0, left: 0, right: 0, height: 32, zIndex: 20, background: 'transparent' }} />
  );
}

export function DragSpacer({ bg }: { bg?: string }) {
  return <div data-tauri-drag-region style={{ height: 28, flexShrink: 0, background: bg }} />;
}

export function NoticeToast({ notice, onClose, p }: {
  notice: AppNotice | null; onClose: () => void; p: Palette;
}) {
  if (!notice) return null;
  const isError = notice.tone === 'error';
  return (
    <div style={{
      position: 'absolute', top: 38, right: 18, zIndex: 40, maxWidth: 420,
      display: 'flex', alignItems: 'flex-start', gap: 10,
      padding: '10px 12px', borderRadius: 8,
      background: p.panel, border: '1px solid ' + (isError ? p.danger : p.line),
      boxShadow: '0 12px 40px rgba(0,0,0,0.16)', color: p.text,
      fontSize: 12, lineHeight: 1.45, whiteSpace: 'pre-line',
    }}>
      <div style={{ width: 8, height: 8, borderRadius: 4, marginTop: 5, background: isError ? p.danger : p.accent, flexShrink: 0 }} />
      <div style={{ flex: 1, minWidth: 0, overflowWrap: 'anywhere' }}>{notice.message}</div>
      <button onClick={onClose} aria-label="Dismiss notice" style={{
        border: 0, background: 'transparent', color: p.text3, cursor: 'pointer',
        fontSize: 13, lineHeight: 1, padding: 2, fontFamily: FONT_SANS,
      }}>×</button>
    </div>
  );
}

export function Section({ title, badge, open = true, setOpen, children, action, p }: {
  title: string; badge?: string; open?: boolean; setOpen?: (b: boolean) => void;
  children: ReactNode; action?: ReactNode; p: Palette;
}) {
  const collapsible = !!setOpen;
  return (
    <div style={{ borderBottom: '1px solid ' + p.lineSoft }}>
      <div onClick={collapsible && setOpen ? () => setOpen(!open) : undefined}
        style={{ padding: '14px 28px 8px', display: 'flex', alignItems: 'center', gap: 8, cursor: collapsible ? 'pointer' : 'default', userSelect: 'none' }}>
        {collapsible && (
          <span style={{ fontSize: 10, color: p.text3, transition: 'transform 0.15s', transform: open ? 'rotate(90deg)' : 'rotate(0)', display: 'inline-block' }}>▶</span>
        )}
        <span style={{ fontSize: 11, fontWeight: 700, color: p.text2, letterSpacing: 0.4, textTransform: 'uppercase' }}>{title}</span>
        {badge && <span style={{ fontSize: 10, color: p.text3, fontFamily: MONO }}>{badge}</span>}
        <div style={{ flex: 1 }} />
        {action}
      </div>
      {open && children}
    </div>
  );
}

export function Toggle({ on, disabled, onClick, p }: {
  on: boolean; disabled?: boolean; onClick: () => void; p: Palette;
}) {
  return (
    <button type="button" onClick={(event) => { event.stopPropagation(); onClick(); }}
      disabled={disabled} aria-pressed={on}
      style={{
        width: 34, height: 20, borderRadius: 10, border: 0, padding: 0,
        backgroundColor: on ? p.hub : 'rgba(120,120,128,0.32)',
        opacity: disabled ? 0.6 : 1, cursor: disabled ? 'not-allowed' : 'pointer',
        position: 'relative', transition: 'background-color 0.15s', flexShrink: 0,
      }}>
      <div style={{
        width: 16, height: 16, borderRadius: 8, background: '#fff',
        position: 'absolute', top: 2, left: on ? 16 : 2,
        transition: 'left 0.15s', boxShadow: '0 1px 2px rgba(0,0,0,0.2)',
      }} />
    </button>
  );
}

export function Segmented<T extends string>({ value, options, onChange, p }: {
  value: T; options: { value: T; label: string }[]; onChange: (v: T) => void; p: Palette;
}) {
  return (
    <div style={{ display: 'inline-flex', background: 'rgba(0,0,0,0.05)', borderRadius: 6, padding: 2 }}>
      {options.map((o) => {
        const selected = value === o.value;
        return (
          <button key={o.value} onClick={() => onChange(o.value)}
            style={{
              border: 0, background: selected ? p.panel : 'transparent', cursor: 'pointer',
              borderRadius: 4, padding: '4px 12px', fontSize: 12, color: p.text, fontFamily: FONT_SANS,
              fontWeight: selected ? 500 : 400, boxShadow: selected ? '0 1px 2px rgba(0,0,0,0.08)' : 'none',
            }}>{o.label}</button>
        );
      })}
    </div>
  );
}

export function Button({ children, onClick, disabled, primary, danger, title, p }: {
  children: ReactNode; onClick: () => void; disabled?: boolean; primary?: boolean; danger?: boolean; title?: string; p: Palette;
}) {
  const filled = primary || danger;
  return (
    <button type="button" title={title} disabled={disabled}
      onClick={(event) => { event.stopPropagation(); onClick(); }}
      style={{
        border: filled ? 0 : '1px solid ' + p.line,
        background: danger ? p.danger : primary ? p.accent : p.panel,
        color: filled ? '#fff' : p.text, borderRadius: 6, padding: '5px 12px',
        fontSize: 12, fontWeight: 500, fontFamily: FONT_SANS, whiteSpace: 'nowrap',
        cursor: disabled ? 'not-allowed' : 'pointer', opacity: disabled ? 0.55 : 1, minHeight: 28,
      }}>{children}</button>
  );
}

/** Two-click confirmation; resets after a few seconds. */
export function useConfirm(resetKey: unknown) {
  const [armed, setArmed] = useState(false);
  useEffect(() => { setArmed(false); }, [resetKey]);
  useEffect(() => {
    if (!armed) return;
    const t = window.setTimeout(() => setArmed(false), 4000);
    return () => window.clearTimeout(t);
  }, [armed]);
  return {
    armed,
    run: (action: () => void) => {
      if (armed) { setArmed(false); action(); } else setArmed(true);
    },
  };
}

export function StateBadge({ state, p }: { state: SkillState; p: Palette }) {
  const tone = state === 'released'
    ? { color: p.hubText, bg: p.hubSoft }
    : state === 'devAhead'
      ? { color: '#a05a00', bg: 'rgba(255,159,10,0.14)' }
      : state === 'devOnly'
        ? { color: p.text2, bg: 'rgba(0,0,0,0.06)' }
        : { color: p.danger, bg: 'rgba(255,59,48,0.08)' };
  return (
    <span style={{ fontSize: 10, padding: '1px 6px', borderRadius: 8, background: tone.bg, color: tone.color, fontWeight: 600, whiteSpace: 'nowrap' }}>
      {STATE_LABELS[state]}
    </span>
  );
}

export function AgentChips({ agents, p }: { agents: string[]; p: Palette }) {
  return (
    <>
      {agents.map((a) => (
        <span key={a} title={agentName(a)} style={{
          fontSize: 9, padding: '1px 6px', borderRadius: 8, background: 'rgba(0,0,0,0.06)',
          color: AGENT_ACCENTS[a] ?? p.text2, fontWeight: 600, letterSpacing: 0.2,
        }}>{agentName(a)}</span>
      ))}
    </>
  );
}

/** One-line human summary of a sync / publish result. */
export function describeSync(r: SyncResult) {
  const parts: string[] = [];
  const count = (a: string) => r.changes.filter((c) => c.action === a).length;
  if (count('created')) parts.push(`新建链接 ${count('created')}`);
  if (count('fixed')) parts.push(`修正 ${count('fixed')}`);
  if (count('removed')) parts.push(`移除 ${count('removed')}`);
  const conflicts = r.changes.filter((c) => c.action === 'conflict');
  const lines = [parts.length ? parts.join(' · ') : '链接无变化'];
  conflicts.forEach((c) => lines.push(`冲突 ${agentName(c.agent)}/${c.name}：${c.message}`));
  r.warnings.forEach((w) => lines.push(w));
  return { text: lines.join('\n'), hasConflict: conflicts.length > 0 };
}
