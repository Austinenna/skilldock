import { useCallback, useEffect, useMemo, useState, type MouseEvent } from 'react';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { api } from './ipc';
import { PALETTES, PALETTE_OPTIONS, PALETTE_KEYS, type Palette } from './palettes';
import type { AiProvider, Config, PaletteKey, Platform, Skill, SkillDetail } from './types';

const FONT_SANS = '-apple-system, BlinkMacSystemFont, "SF Pro Text", "Helvetica Neue", "Inter", sans-serif';
const FONT_DISPLAY_WARM = '"Instrument Serif", Georgia, serif';
const MONO = '"JetBrains Mono", "SF Mono", Menlo, monospace';
const AI_PRESETS: Record<AiProvider, { endpoint: string; model: string }> = {
  anthropic: {
    endpoint: 'https://api.minimaxi.com/anthropic/v1/messages',
    model: 'MiniMax-M2.7',
  },
  chat: {
    endpoint: 'https://token-plan-sgp.xiaomimimo.com/v1/chat/completions',
    model: 'mimo-v2.5-pro',
  },
};

const PLATFORM_ACCENTS: Record<string, string> = {
  central: '#34c759', claude: '#bf5af2', codex: '#5e9eff', openclaw: '#ff9f0a',
  cursor: '#a0a0a0', gemini: '#4285f4', copilot: '#1f2328', windsurf: '#2dd4bf',
  aider: '#dc2626', qclaw: '#ef4444', easyclaw: '#fbbf24', workbuddy: '#10b981',
};
const PLATFORM_ICONS: Record<string, string> = {
  central: '◎', claude: 'C', codex: 'X', openclaw: '🦞', cursor: 'c',
  gemini: 'G', copilot: 'p', windsurf: 'W', aider: 'A',
  qclaw: 'q', easyclaw: 'E', workbuddy: 'W',
};

type NoticeTone = 'error' | 'info';

interface AppNotice {
  id: number;
  tone: NoticeTone;
  message: string;
}

type AiTestState = { tone: NoticeTone; message: string } | null;

// ─── small helpers ────────────────────────────────────────────────
function useFontLink() {
  useEffect(() => {
    if (document.getElementById('skillloom-fonts')) return;
    const l = document.createElement('link');
    l.id = 'skillloom-fonts';
    l.rel = 'stylesheet';
    l.href = 'https://fonts.googleapis.com/css2?family=Instrument+Serif:ital@0;1&family=JetBrains+Mono:wght@400;500&display=swap';
    document.head.appendChild(l);
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

function WindowDragStrip() {
  return (
    <div
      data-tauri-drag-region
      onMouseDown={startWindowDrag}
      style={{
        position: 'absolute',
        top: 0,
        left: 0,
        right: 0,
        height: 32,
        zIndex: 20,
        background: 'transparent',
      }}
    />
  );
}

function NoticeToast({ notice, onClose, p }: {
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
      fontSize: 12, lineHeight: 1.45,
    }}>
      <div style={{
        width: 8, height: 8, borderRadius: 4, marginTop: 5,
        background: isError ? p.danger : p.accent, flexShrink: 0,
      }} />
      <div style={{ flex: 1, minWidth: 0, overflowWrap: 'anywhere' }}>{notice.message}</div>
      <button onClick={onClose} aria-label="Dismiss notice" style={{
        border: 0, background: 'transparent', color: p.text3, cursor: 'pointer',
        fontSize: 13, lineHeight: 1, padding: 2, fontFamily: FONT_SANS,
      }}>×</button>
    </div>
  );
}

// ─── Sidebar ──────────────────────────────────────────────────────
function SidebarSection({ title, p, children }: { title: string; p: Palette; children: React.ReactNode }) {
  return (
    <div style={{ padding: '14px 0 4px' }}>
      <div style={{ padding: '0 16px 6px', fontSize: 11, fontWeight: 700, color: p.text3, letterSpacing: 0.2, textTransform: 'uppercase' }}>{title}</div>
      {children}
    </div>
  );
}

function SidebarItem({ icon, label, count, active, accent, onClick, p }: {
  icon: string; label: string; count?: number; active: boolean; accent?: string; onClick: () => void; p: Palette;
}) {
  const [hover, setHover] = useState(false);
  return (
    <div onClick={onClick}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        margin: '0 8px', padding: '5px 8px', borderRadius: 6,
        display: 'flex', alignItems: 'center', gap: 8,
        fontSize: 13, color: active ? '#fff' : p.text,
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

function Sidebar({ active, setActive, counts, visiblePlatforms, totalSkills, p }: {
  active: string; setActive: (s: string) => void; counts: Record<string, number>;
  visiblePlatforms: Platform[]; totalSkills: number; p: Palette;
}) {
  const hub = visiblePlatforms.find((pl) => pl.isHub);
  const routes = visiblePlatforms.filter((pl) => !pl.isHub);
  return (
    <div style={{ width: 200, background: p.sidebar, borderRight: '1px solid ' + p.line, display: 'flex', flexDirection: 'column', flexShrink: 0 }}>
      <div data-tauri-drag-region style={{ height: 28, flexShrink: 0 }} />
      <div style={{ flex: 1, overflowY: 'auto', paddingBottom: 12 }}>
        {hub && (
          <SidebarSection title="Hub" p={p}>
            <SidebarItem icon={PLATFORM_ICONS[hub.id] ?? '◎'} label={hub.name} count={counts[hub.id]}
              accent={PLATFORM_ACCENTS[hub.id]} active={active === hub.id} onClick={() => setActive(hub.id)} p={p} />
          </SidebarSection>
        )}
        {routes.length > 0 && (
          <SidebarSection title="Routes" p={p}>
            {routes.map((pl) => (
              <SidebarItem key={pl.id} icon={PLATFORM_ICONS[pl.id] || pl.short[0]} label={pl.name} count={counts[pl.id]}
                accent={PLATFORM_ACCENTS[pl.id] || p.text3}
                active={active === pl.id} onClick={() => setActive(pl.id)} p={p} />
            ))}
          </SidebarSection>
        )}
        <SidebarSection title="Tools" p={p}>
          <SidebarItem icon="⚙" label="Settings" accent="#8e8e93"
            active={active === 'settings'} onClick={() => setActive('settings')} p={p} />
        </SidebarSection>
      </div>
      <div style={{ padding: '8px 12px', fontSize: 11, color: p.text3, borderTop: '1px solid ' + p.lineSoft, display: 'flex', alignItems: 'center', gap: 6 }}>
        <span style={{ width: 6, height: 6, borderRadius: 3, background: p.hub }} />
        {totalSkills} skills · {visiblePlatforms.length - (hub ? 1 : 0)} routes
      </div>
    </div>
  );
}

// ─── List header / rows / cards ────────────────────────────────────
function ListHeader({ title, q, setQ, view, setView, onImport, onRefresh, refreshing, importDisabled, p, density }: {
  title: string; q: string; setQ: (s: string) => void; view: 'list' | 'grid';
  setView: (v: 'list' | 'grid') => void; onImport: () => void; onRefresh: () => void;
  refreshing: boolean; importDisabled: boolean; p: Palette; density: string;
}) {
  return (
    <div style={{ borderBottom: '1px solid ' + p.line, padding: density === 'compact' ? '8px 12px' : '10px 12px 10px 14px', display: 'flex', flexDirection: 'column', gap: 8, background: p.panel }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <div style={{ fontSize: 13, fontWeight: 600, color: p.text }}>{title}</div>
        <div style={{ flex: 1 }} />
        <button onClick={onRefresh} disabled={refreshing} style={{
          border: '1px solid ' + p.line, background: p.panel, color: p.text, cursor: refreshing ? 'not-allowed' : 'pointer',
          borderRadius: 6, padding: '4px 9px', fontSize: 12, fontWeight: 500, fontFamily: FONT_SANS,
          opacity: refreshing ? 0.6 : 1,
        }}>{refreshing ? 'Refreshing' : 'Refresh'}</button>
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
        <button onClick={onImport} disabled={importDisabled} style={{
          border: 0, background: p.accent, color: '#fff', cursor: importDisabled ? 'not-allowed' : 'pointer',
          borderRadius: 6, padding: '4px 10px', fontSize: 12, fontWeight: 500, fontFamily: FONT_SANS,
          display: 'flex', alignItems: 'center', gap: 4, opacity: importDisabled ? 0.6 : 1,
        }}>＋ Import</button>
      </div>
      <div style={{ position: 'relative' }}>
        <span style={{ position: 'absolute', left: 8, top: '50%', transform: 'translateY(-50%)', color: p.text3, fontSize: 12 }}>⌕</span>
        <input value={q} onChange={(e) => setQ(e.target.value)} placeholder="Filter skills…"
          style={{
            width: '100%', boxSizing: 'border-box', border: 'none', background: 'rgba(0,0,0,0.05)',
            borderRadius: 6, padding: '5px 8px 5px 24px', fontSize: 12, color: p.text, outline: 'none',
            fontFamily: FONT_SANS,
          }} />
      </div>
    </div>
  );
}

function chipFor(routes: string[], platforms: Platform[], p: Palette) {
  return routes.slice(0, 4).map((r) => {
    const pl = platforms.find((x) => x.id === r);
    return (
      <span key={r} title={pl?.name}
        style={{
          fontSize: 9, padding: '1px 6px', borderRadius: 8,
          background: r === 'central' ? p.hubSoft : 'rgba(0,0,0,0.06)',
          color: r === 'central' ? p.hubText : p.text2,
          fontWeight: 500, letterSpacing: 0.2,
        }}>{pl?.short ?? r}</span>
    );
  });
}

function SkillRow({ skill, selected, onClick, p, density, platforms }: {
  skill: Skill; selected: boolean; onClick: () => void; p: Palette; density: string; platforms: Platform[];
}) {
  const [hover, setHover] = useState(false);
  const pad = density === 'compact' ? '7px 14px' : '10px 14px';
  return (
    <div onClick={onClick}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
      style={{
        padding: pad, borderBottom: '1px solid ' + p.lineSoft,
        backgroundColor: selected ? p.rowSel : (hover ? p.rowHover : 'transparent'),
        cursor: 'pointer', display: 'flex', gap: 10, alignItems: 'flex-start',
      }}>
      <div style={{
        width: 28, height: 28, borderRadius: 6, background: p.iconBg,
        color: p.text2, fontSize: 13, fontWeight: 700, fontFamily: MONO,
        display: 'grid', placeItems: 'center', flexShrink: 0, marginTop: 2,
      }}>{skill.title[0]}</div>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ display: 'flex', alignItems: 'baseline', gap: 6 }}>
          <span style={{ fontSize: 13, fontWeight: 500, color: p.text, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{skill.title}</span>
          {skill.version && <span style={{ fontSize: 10, color: p.text3, fontFamily: MONO }}>{skill.version}</span>}
        </div>
        <div style={{ fontSize: 12, color: p.text2, marginTop: 2, lineHeight: 1.4, overflow: 'hidden', textOverflow: 'ellipsis', display: '-webkit-box', WebkitLineClamp: 1, WebkitBoxOrient: 'vertical' }}>
          {skill.tagline || <span style={{ color: p.text3, fontStyle: 'italic' }}>No description</span>}
        </div>
        {density !== 'compact' && (
          <div style={{ display: 'flex', gap: 3, marginTop: 6 }}>{chipFor(skill.routes, platforms, p)}</div>
        )}
      </div>
      <div style={{ width: 4, height: 4, borderRadius: 2, background: p.hub, marginTop: 8, flexShrink: 0 }} />
    </div>
  );
}

function SkillCard({ skill, selected, onClick, p, platforms }: {
  skill: Skill; selected: boolean; onClick: () => void; p: Palette; platforms: Platform[];
}) {
  return (
    <div onClick={onClick}
      style={{
        padding: 12, background: selected ? p.rowSel : p.panel,
        border: '1px solid ' + (selected ? p.accent : p.line),
        borderRadius: 8, cursor: 'pointer', display: 'flex', flexDirection: 'column', gap: 6,
      }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <div style={{ width: 22, height: 22, borderRadius: 5, background: p.iconBg, color: p.text2, fontSize: 11, fontWeight: 700, fontFamily: MONO, display: 'grid', placeItems: 'center' }}>{skill.title[0]}</div>
        <div style={{ fontSize: 12, fontWeight: 600, color: p.text, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{skill.title}</div>
      </div>
      <div style={{ fontSize: 11, color: p.text2, lineHeight: 1.4, overflow: 'hidden', display: '-webkit-box', WebkitLineClamp: 2, WebkitBoxOrient: 'vertical' }}>
        {skill.tagline || <span style={{ color: p.text3, fontStyle: 'italic' }}>No description</span>}
      </div>
      <div style={{ display: 'flex', gap: 3 }}>{chipFor(skill.routes, platforms, p)}</div>
      <div style={{ fontSize: 10, color: p.text3, fontFamily: MONO }}>{skill.routes.length} routes · {skill.size}</div>
    </div>
  );
}

// ─── Detail ───────────────────────────────────────────────────────
function Section({ title, badge, open = true, setOpen, children, action, p }: {
  title: string; badge?: string; open?: boolean; setOpen?: (b: boolean) => void;
  children: React.ReactNode; action?: React.ReactNode; p: Palette;
}) {
  const collapsible = !!setOpen;
  return (
    <div style={{ borderBottom: '1px solid ' + p.lineSoft }}>
      <div onClick={collapsible && setOpen ? () => setOpen(!open) : undefined}
        style={{
          padding: '14px 28px 8px', display: 'flex', alignItems: 'center', gap: 8,
          cursor: collapsible ? 'pointer' : 'default', userSelect: 'none',
        }}>
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

function Toggle({ on, disabled, blocked, onClick, p }: {
  on: boolean; disabled?: boolean; blocked?: boolean; onClick: () => void; p: Palette;
}) {
  const unavailable = disabled || blocked;
  return (
    <button
      type="button"
      onClick={(event) => { event.stopPropagation(); onClick(); }}
      disabled={disabled}
      aria-pressed={on}
      aria-disabled={unavailable}
      style={{
        width: 34, height: 20, borderRadius: 10, border: 0, padding: 0,
        backgroundColor: on ? p.hub : 'rgba(120,120,128,0.32)',
        opacity: unavailable ? 0.6 : 1, cursor: disabled ? 'not-allowed' : 'pointer',
        position: 'relative', transition: 'background-color 0.15s',
      }}>
      <div style={{
        width: 16, height: 16, borderRadius: 8, background: '#fff',
        position: 'absolute', top: 2, left: on ? 16 : 2,
        transition: 'left 0.15s', boxShadow: '0 1px 2px rgba(0,0,0,0.2)',
      }} />
    </button>
  );
}

function Detail({ skill, detail, detailLoading, detailError, summary, summaryLoading, summaryError, onRegenerateSummary, toggleRoute, onDelete, onNotice, routePending, deletePending, visiblePlatforms, p, paletteKey }: {
  skill: Skill; detail?: SkillDetail; detailLoading: boolean; detailError: string | null;
  summary?: string; summaryLoading: boolean; summaryError: string | null; onRegenerateSummary: () => void;
  toggleRoute: (pid: string) => void; onDelete: () => void; onNotice: (message: string, tone?: NoticeTone) => void;
  routePending: string | null; deletePending: boolean;
  visiblePlatforms: Platform[]; p: Palette; paletteKey: PaletteKey;
}) {
  const [descOpen, setDescOpen] = useState(true);
  const [filesOpen, setFilesOpen] = useState(false);
  const [confirmDel, setConfirmDel] = useState(false);
  const display = paletteKey === 'warm' ? FONT_DISPLAY_WARM : FONT_SANS;
  const titleSize = paletteKey === 'warm' ? 28 : 19;
  const titleWeight = paletteKey === 'warm' ? 400 : 600;

  useEffect(() => { setConfirmDel(false); }, [skill.id]);

  const fileBadge = detail ? String(detail.files.length) : (detailLoading ? 'loading' : String(skill.files));
  const summaryBoxIsError = Boolean(summaryError);

  return (
    <div style={{ flex: 1, overflowY: 'auto', background: p.panel }}>
      <div style={{ padding: '24px 28px 16px', borderBottom: '1px solid ' + p.lineSoft }}>
        <div style={{ display: 'flex', alignItems: 'flex-start', gap: 14 }}>
          <div style={{
            width: 52, height: 52, borderRadius: 12, background: p.iconLg,
            color: p.iconTextLg, fontSize: 22, fontWeight: 700, fontFamily: MONO,
            display: 'grid', placeItems: 'center',
            boxShadow: '0 4px 12px rgba(0,0,0,0.08)', flexShrink: 0,
          }}>{skill.title[0].toUpperCase()}</div>
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{ fontSize: titleSize, fontWeight: titleWeight, color: p.text, letterSpacing: -0.3, fontFamily: display, lineHeight: 1.1 }}>{skill.title}</div>
            <div style={{ fontSize: 13, color: p.text2, marginTop: 4 }}>{skill.tagline || <span style={{ color: p.text3, fontStyle: 'italic' }}>No description</span>}</div>
            <div style={{ display: 'flex', gap: 12, marginTop: 8, fontSize: 11, color: p.text3, fontFamily: MONO, flexWrap: 'wrap' }}>
              {skill.version && (<><span>v{skill.version}</span><span>·</span></>)}
              <span>{skill.files} files</span>
              <span>·</span>
              <span>{skill.size}</span>
              {skill.updated && (<><span>·</span><span>updated {skill.updated}</span></>)}
            </div>
          </div>
          <button disabled={deletePending} onClick={() => {
            if (confirmDel) { onDelete(); setConfirmDel(false); }
            else setConfirmDel(true);
          }}
            style={{
              border: '1px solid ' + (confirmDel ? p.danger : p.line),
              background: confirmDel ? p.danger : p.panel,
              color: confirmDel ? '#fff' : p.danger, fontSize: 12, fontFamily: FONT_SANS,
              borderRadius: 6, padding: '5px 10px', cursor: deletePending ? 'not-allowed' : 'pointer',
              transition: 'background-color 0.12s, color 0.12s', opacity: deletePending ? 0.6 : 1,
            }}>{deletePending ? 'Deleting' : (confirmDel ? 'Confirm delete' : 'Delete')}</button>
        </div>
      </div>

      <Section title="AI Summary" badge={summaryLoading ? 'loading' : (summary && summary !== skill.tagline ? 'ready' : 'fallback')} p={p} open={descOpen} setOpen={setDescOpen}
        action={(
          <button onClick={(event) => { event.stopPropagation(); onRegenerateSummary(); }} disabled={summaryLoading}
            style={{
              border: '1px solid ' + p.line, background: p.panel, color: p.text2,
              borderRadius: 6, padding: '4px 9px', fontSize: 11, cursor: summaryLoading ? 'not-allowed' : 'pointer',
              opacity: summaryLoading ? 0.6 : 1, fontFamily: FONT_SANS,
            }}>{summaryLoading ? 'Generating' : 'Regenerate'}</button>
        )}>
        <div style={{ padding: '4px 28px 20px' }}>
          <div style={{
            fontSize: 13, lineHeight: 1.6, color: summaryBoxIsError ? p.danger : p.text3,
            padding: '14px 16px', borderRadius: 10,
            background: summaryBoxIsError ? 'rgba(255,59,48,0.06)' : 'linear-gradient(180deg, ' + p.accentSoft + ', ' + p.accentSoft.replace(/0\.\d+/, '0.03') + ')',
            border: '1px solid ' + (summaryBoxIsError ? 'rgba(255,59,48,0.22)' : p.accentSoft),
            fontStyle: paletteKey === 'warm' ? 'italic' : 'normal',
            fontFamily: paletteKey === 'warm' ? FONT_DISPLAY_WARM : FONT_SANS,
            ...(paletteKey === 'warm' ? { fontSize: 16 } : {}),
          }}>
            {summaryLoading
              ? 'Generating summary…'
              : summaryError
                ? summaryError
                : summary || skill.tagline || 'No summary available.'}
          </div>
          {skill.tags.length > 0 && (
            <div style={{ display: 'flex', gap: 6, marginTop: 10 }}>
              {skill.tags.map((t) => (
                <span key={t} style={{ fontSize: 11, padding: '2px 8px', borderRadius: 10, background: 'rgba(0,0,0,0.05)', color: p.text2, fontFamily: MONO }}>#{t}</span>
              ))}
            </div>
          )}
        </div>
      </Section>

      <Section title="Routing" badge={`${skill.routes.length} active`} p={p}>
        <div style={{ padding: '4px 28px 20px', display: 'flex', flexDirection: 'column', gap: 6 }}>
          {visiblePlatforms.map((pl) => {
            const on = skill.routes.includes(pl.id);
            const conflict = skill.routeConflicts.find((item) => item.platformId === pl.id);
            const pending = routePending === pl.id;
            const handleRouteClick = () => {
              if (pl.isHub || pending) return;
              if (conflict) {
                onNotice(conflict.message, 'error');
                return;
              }
              toggleRoute(pl.id);
            };
            return (
              <div
                key={pl.id}
                onClick={handleRouteClick}
                onKeyDown={(event) => {
                  if (event.key === 'Enter' || event.key === ' ') {
                    event.preventDefault();
                    handleRouteClick();
                  }
                }}
                role={pl.isHub ? undefined : 'button'}
                tabIndex={pl.isHub ? undefined : 0}
                title={conflict?.message}
                style={{
                display: 'flex', alignItems: 'center', gap: 12,
                padding: '10px 12px', border: '1px solid ' + p.line, borderRadius: 8,
                background: on ? 'transparent' : 'rgba(0,0,0,0.015)',
                cursor: pl.isHub ? 'default' : (pending ? 'not-allowed' : 'pointer'),
              }}>
                <div style={{ width: 24, height: 24, borderRadius: 6, background: pl.isHub ? p.hubSoft : 'rgba(0,0,0,0.06)', color: p.text2, fontSize: 12, fontWeight: 700, display: 'grid', placeItems: 'center', fontFamily: MONO, flexShrink: 0 }}>{pl.short[0]}</div>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontSize: 13, color: p.text, fontWeight: 500, display: 'flex', alignItems: 'center', gap: 6 }}>
                    {pl.name}
                    {pl.isHub && <span style={{ fontSize: 9, color: p.hubText, background: p.hubSoft, padding: '1px 5px', borderRadius: 3, fontWeight: 700 }}>HUB</span>}
                    {conflict && <span style={{ fontSize: 9, color: p.danger, background: 'rgba(255,59,48,0.1)', padding: '1px 5px', borderRadius: 3, fontWeight: 700 }}>CONFLICT</span>}
                  </div>
                  <div style={{ fontSize: 11, color: conflict ? p.danger : p.text3, fontFamily: MONO, marginTop: 2, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{pl.path}{skill.id}</div>
                </div>
                <Toggle on={on} disabled={pl.isHub || pending} blocked={!!conflict} onClick={handleRouteClick} p={p} />
              </div>
            );
          })}
        </div>
      </Section>

      <Section title="Files" badge={fileBadge} p={p} open={filesOpen} setOpen={setFilesOpen}>
        <div style={{ padding: '4px 28px 24px', fontFamily: MONO, fontSize: 12, color: p.text2 }}>
          {detailLoading ? (
            <div style={{ color: p.text3 }}>Loading file details…</div>
          ) : detailError ? (
            <div style={{ color: p.danger }}>{detailError}</div>
          ) : detail ? (
            <>
              <div style={{ color: p.text3, marginBottom: 10, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                {detail.sourcePath}
              </div>
              {detail.files.length === 0 ? (
                <div style={{ color: p.text3 }}>Empty skill directory.</div>
              ) : (
                <div style={{ border: '1px solid ' + p.line, borderRadius: 8, overflow: 'hidden' }}>
                  {detail.files.map((file, index) => (
                    <div key={file.name} style={{
                      display: 'grid', gridTemplateColumns: 'minmax(0, 1fr) 78px 70px 92px',
                      gap: 10, alignItems: 'center', padding: '8px 10px',
                      borderBottom: index < detail.files.length - 1 ? '1px solid ' + p.lineSoft : 'none',
                      background: index % 2 === 0 ? 'transparent' : 'rgba(0,0,0,0.015)',
                    }}>
                      <div title={file.name} style={{ color: p.text, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{file.name}</div>
                      <div style={{ color: p.text3 }}>{file.kind}</div>
                      <div style={{ color: p.text3, textAlign: 'right' }}>{file.size || '--'}</div>
                      <div style={{ color: p.text3, textAlign: 'right', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{file.modified || '--'}</div>
                    </div>
                  ))}
                </div>
              )}
            </>
          ) : (
            <div style={{ color: p.text3 }}>File details unavailable.</div>
          )}
        </div>
      </Section>
    </div>
  );
}

// ─── Segmented control (Settings) ─────────────────────────────────
function Segmented<T extends string>({ value, options, onChange, p }: {
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
              fontWeight: selected ? 500 : 400,
              boxShadow: selected ? '0 1px 2px rgba(0,0,0,0.08)' : 'none',
            }}>{o.label}</button>
        );
      })}
    </div>
  );
}

// ─── Settings ─────────────────────────────────────────────────────
function SettingsPane({ p, platforms, config, setConfig, apiKeyConfigured, apiKeyPending, aiTestPending, aiTest, onSaveApiKey, onClearApiKey, onTestAi }: {
  p: Palette; platforms: Platform[]; config: Config; setConfig: (patch: Partial<Config>) => Promise<void>;
  apiKeyConfigured: boolean; apiKeyPending: boolean; aiTestPending: boolean; aiTest: AiTestState;
  onSaveApiKey: (key: string) => void; onClearApiKey: () => void; onTestAi: () => Promise<void>;
}) {
  const [apiKey, setApiKey] = useState('');
  const [aiEndpoint, setAiEndpoint] = useState(config.aiEndpoint);
  const [aiModel, setAiModel] = useState(config.aiModel);
  useEffect(() => {
    setAiEndpoint(config.aiEndpoint);
    setAiModel(config.aiModel);
  }, [config.aiEndpoint, config.aiModel]);
  const toggleHidden = (id: string) => {
    const isHidden = config.hiddenPlatforms.includes(id);
    setConfig({
      hiddenPlatforms: isHidden
        ? config.hiddenPlatforms.filter((x) => x !== id)
        : [...config.hiddenPlatforms, id],
    });
  };
  const changeAiProvider = (provider: AiProvider) => {
    const preset = AI_PRESETS[provider];
    setAiEndpoint(preset.endpoint);
    setAiModel(preset.model);
    setConfig({ aiProvider: provider, aiEndpoint: preset.endpoint, aiModel: preset.model });
  };
  useEffect(() => {
    const endpoint = aiEndpoint.trim();
    const model = aiModel.trim();
    if (endpoint === config.aiEndpoint && model === config.aiModel) return;
    const timeout = window.setTimeout(() => {
      setConfig({ aiEndpoint: endpoint, aiModel: model });
    }, 500);
    return () => window.clearTimeout(timeout);
  }, [aiEndpoint, aiModel, config.aiEndpoint, config.aiModel, setConfig]);
  const testCurrentAiConfig = async () => {
    await setConfig({ aiEndpoint: aiEndpoint.trim(), aiModel: aiModel.trim() });
    await onTestAi();
  };

  return (
    <div style={{ flex: 1, overflowY: 'auto', background: p.panel }}>
      <div data-tauri-drag-region style={{ height: 28, flexShrink: 0 }} />
      <div style={{ padding: '4px 40px 32px' }}>
        <div style={{ fontSize: 22, fontWeight: 600, color: p.text, letterSpacing: -0.3 }}>Settings</div>
        <div style={{ fontSize: 13, color: p.text2, marginTop: 4, marginBottom: 24 }}>
          Manage appearance and which platforms appear in the sidebar.
        </div>

        {/* Appearance */}
        <div style={{ marginBottom: 32 }}>
          <div style={{ fontSize: 11, fontWeight: 700, color: p.text3, letterSpacing: 0.4, textTransform: 'uppercase', marginBottom: 12 }}>Appearance</div>
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: 12 }}>
            {PALETTE_KEYS.map((key) => {
              const pal = PALETTES[key];
              const swatches = PALETTE_OPTIONS[PALETTE_KEYS.indexOf(key)];
              const selected = config.palette === key;
              return (
                <button key={key} onClick={() => setConfig({ palette: key })}
                  style={{
                    border: '1px solid ' + (selected ? pal.accent : p.line),
                    background: pal.panel, borderRadius: 10, padding: 0, cursor: 'pointer',
                    textAlign: 'left', overflow: 'hidden', fontFamily: FONT_SANS,
                    boxShadow: selected ? '0 0 0 2px ' + pal.accentSoft : 'none',
                    transition: 'box-shadow 0.15s, border-color 0.15s',
                  }}>
                  <div style={{ height: 80, background: pal.sidebarFlat, position: 'relative', display: 'flex' }}>
                    <div style={{ width: 28, height: '100%', background: pal.sidebarFlat, borderRight: '1px solid ' + pal.line, display: 'flex', flexDirection: 'column', alignItems: 'center', paddingTop: 8, gap: 4 }}>
                      <div style={{ width: 4, height: 4, borderRadius: 2, background: pal.hub }} />
                      <div style={{ width: 4, height: 4, borderRadius: 2, background: pal.accent }} />
                      <div style={{ width: 4, height: 4, borderRadius: 2, background: pal.text3 }} />
                    </div>
                    <div style={{ flex: 1, padding: '8px 10px', background: pal.panel }}>
                      <div style={{ height: 5, width: '60%', background: pal.text, borderRadius: 2, opacity: 0.85 }} />
                      <div style={{ height: 4, width: '80%', background: pal.text2, borderRadius: 2, marginTop: 4, opacity: 0.5 }} />
                      <div style={{ display: 'flex', gap: 3, marginTop: 6 }}>
                        <span style={{ width: 16, height: 5, background: pal.hubSoft, borderRadius: 2 }} />
                        <span style={{ width: 16, height: 5, background: pal.accentSoft, borderRadius: 2 }} />
                      </div>
                    </div>
                    {selected && (
                      <div style={{ position: 'absolute', top: 6, right: 6, width: 16, height: 16, borderRadius: 8, background: pal.accent, color: '#fff', display: 'grid', placeItems: 'center', fontSize: 10, fontWeight: 700 }}>✓</div>
                    )}
                  </div>
                  <div style={{ padding: '10px 12px', display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 8, borderTop: '1px solid ' + pal.lineSoft }}>
                    <div>
                      <div style={{ fontSize: 12, fontWeight: 600, color: pal.text }}>{pal.name.split(' · ')[0]}</div>
                      <div style={{ fontSize: 10, color: pal.text3, marginTop: 1 }}>{pal.name.split(' · ')[1]}</div>
                    </div>
                    <div style={{ display: 'flex', gap: 2 }}>
                      {swatches.map((c, i) => (
                        <span key={i} style={{ width: 10, height: 10, borderRadius: 5, background: c, boxShadow: 'inset 0 0 0 .5px rgba(0,0,0,0.1)' }} />
                      ))}
                    </div>
                  </div>
                </button>
              );
            })}
          </div>
        </div>

        {/* Preferences */}
        <div style={{ marginBottom: 32 }}>
          <div style={{ fontSize: 11, fontWeight: 700, color: p.text3, letterSpacing: 0.4, textTransform: 'uppercase', marginBottom: 12 }}>Preferences</div>
          <div style={{ border: '1px solid ' + p.line, borderRadius: 8, overflow: 'hidden' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '12px 14px', borderBottom: '1px solid ' + p.lineSoft }}>
              <div style={{ flex: 1, minWidth: 0 }}>
                <div style={{ fontSize: 13, color: p.text, fontWeight: 500 }}>List density</div>
                <div style={{ fontSize: 11, color: p.text3, marginTop: 2 }}>Tighter rows show more skills at once.</div>
              </div>
              <Segmented value={config.density} onChange={(v) => setConfig({ density: v })} p={p}
                options={[{ value: 'comfortable', label: 'Comfortable' }, { value: 'compact', label: 'Compact' }]} />
            </div>
            <div style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '12px 14px' }}>
              <div style={{ flex: 1, minWidth: 0 }}>
                <div style={{ fontSize: 13, color: p.text, fontWeight: 500 }}>Default view</div>
                <div style={{ fontSize: 11, color: p.text3, marginTop: 2 }}>Switchable any time from the list header.</div>
              </div>
              <Segmented value={config.view} onChange={(v) => setConfig({ view: v })} p={p}
                options={[{ value: 'list', label: 'List' }, { value: 'grid', label: 'Grid' }]} />
            </div>
          </div>
        </div>

        {/* AI */}
        <div style={{ marginBottom: 32 }}>
          <div style={{ fontSize: 11, fontWeight: 700, color: p.text3, letterSpacing: 0.4, textTransform: 'uppercase', marginBottom: 12 }}>AI</div>
          <div style={{ border: '1px solid ' + p.line, borderRadius: 8, overflow: 'hidden' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '12px 14px', borderBottom: '1px solid ' + p.lineSoft }}>
              <div style={{ flex: 1, minWidth: 0 }}>
                <div style={{ fontSize: 13, color: p.text, fontWeight: 500 }}>Provider protocol</div>
                <div style={{ fontSize: 11, color: p.text3, marginTop: 2 }}>Choose the request/response shape for summaries.</div>
              </div>
              <Segmented<AiProvider> value={config.aiProvider} onChange={changeAiProvider} p={p}
                options={[{ value: 'anthropic', label: 'Anthropic' }, { value: 'chat', label: 'Chat' }]} />
            </div>
            <div style={{ display: 'grid', gridTemplateColumns: '1fr minmax(320px, 1.2fr)', gap: 10, alignItems: 'center', padding: '12px 14px', borderBottom: '1px solid ' + p.lineSoft }}>
              <div>
                <div style={{ fontSize: 13, color: p.text, fontWeight: 500 }}>Endpoint</div>
                <div style={{ fontSize: 11, color: p.text3, marginTop: 2 }}>Full URL, saved automatically after editing.</div>
              </div>
              <input value={aiEndpoint} onChange={(e) => setAiEndpoint(e.target.value)} placeholder={AI_PRESETS[config.aiProvider].endpoint}
                style={{ width: '100%', boxSizing: 'border-box', border: '1px solid ' + p.line, borderRadius: 6, padding: '6px 9px', fontSize: 11, fontFamily: MONO, outline: 'none', color: p.text, background: p.panel }} />
            </div>
            <div style={{ display: 'grid', gridTemplateColumns: '1fr minmax(320px, 1.2fr)', gap: 10, alignItems: 'center', padding: '12px 14px', borderBottom: '1px solid ' + p.lineSoft }}>
              <div>
                <div style={{ fontSize: 13, color: p.text, fontWeight: 500 }}>Model</div>
                <div style={{ fontSize: 11, color: p.text3, marginTop: 2 }}>Used for testing and summary regeneration.</div>
              </div>
              <input value={aiModel} onChange={(e) => setAiModel(e.target.value)} placeholder={AI_PRESETS[config.aiProvider].model}
                style={{ width: '100%', boxSizing: 'border-box', border: '1px solid ' + p.line, borderRadius: 6, padding: '6px 9px', fontSize: 11, fontFamily: MONO, outline: 'none', color: p.text, background: p.panel }} />
            </div>
            <div style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '12px 14px' }}>
              <div style={{ flex: 1, minWidth: 0 }}>
                <div style={{ fontSize: 13, color: p.text, fontWeight: 500 }}>API key</div>
                <div style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 11, color: apiKeyConfigured ? p.hubText : p.text3, marginTop: 2 }}>
                  <span style={{ width: 6, height: 6, borderRadius: 3, background: apiKeyConfigured ? p.hub : p.text3, display: 'inline-block' }} />
                  {apiKeyConfigured ? 'Saved in macOS Keychain. Hidden for security.' : (config.aiProvider === 'chat' ? 'No key stored. Sent as api-key.' : 'No key stored. Sent as x-api-key.')}
                </div>
              </div>
              <input value={apiKey} disabled={apiKeyPending} type="password" onChange={(e) => setApiKey(e.target.value)} placeholder="Paste API key"
                style={{ width: 220, boxSizing: 'border-box', border: '1px solid ' + p.line, borderRadius: 6, padding: '6px 9px', fontSize: 12, fontFamily: MONO, outline: 'none', color: p.text, background: p.panel }} />
              <button onClick={() => { onSaveApiKey(apiKey); setApiKey(''); }} disabled={apiKeyPending || !apiKey.trim()} style={{
                border: 0, background: p.accent, color: '#fff', borderRadius: 6, padding: '6px 12px',
                fontSize: 12, cursor: apiKeyPending || !apiKey.trim() ? 'not-allowed' : 'pointer',
                opacity: apiKeyPending || !apiKey.trim() ? 0.55 : 1, fontFamily: FONT_SANS,
              }}>{apiKeyPending ? 'Saving' : 'Save'}</button>
              <button onClick={onClearApiKey} disabled={apiKeyPending || !apiKeyConfigured} style={{
                border: '1px solid ' + p.line, background: p.panel, color: p.text, borderRadius: 6, padding: '6px 12px',
                fontSize: 12, cursor: apiKeyPending || !apiKeyConfigured ? 'not-allowed' : 'pointer',
                opacity: apiKeyPending || !apiKeyConfigured ? 0.55 : 1, fontFamily: FONT_SANS,
              }}>Clear</button>
              <button onClick={testCurrentAiConfig} disabled={apiKeyPending || aiTestPending || !apiKeyConfigured} style={{
                border: '1px solid ' + p.line, background: p.panel, color: p.text, borderRadius: 6, padding: '6px 12px',
                fontSize: 12, cursor: apiKeyPending || aiTestPending || !apiKeyConfigured ? 'not-allowed' : 'pointer',
                opacity: apiKeyPending || aiTestPending || !apiKeyConfigured ? 0.55 : 1, fontFamily: FONT_SANS,
              }}>{aiTestPending ? 'Testing' : 'Test'}</button>
            </div>
            {aiTest && (
              <div style={{ display: 'grid', gridTemplateColumns: '1fr minmax(320px, 1.2fr)', gap: 10, alignItems: 'start', padding: '12px 14px', borderTop: '1px solid ' + p.lineSoft }}>
                <div>
                  <div style={{ fontSize: 13, color: p.text, fontWeight: 500 }}>Test result</div>
                  <div style={{ fontSize: 11, color: p.text3, marginTop: 2 }}>Latest connection check.</div>
                </div>
                <div style={{
                  border: '1px solid ' + (aiTest.tone === 'info' ? p.hubSoft : 'rgba(255,59,48,0.22)'),
                  background: aiTest.tone === 'info' ? 'rgba(52,199,89,0.08)' : 'rgba(255,59,48,0.06)',
                  color: aiTest.tone === 'info' ? p.hubText : p.danger,
                  borderRadius: 6, padding: '7px 9px', fontSize: 11, lineHeight: 1.5,
                }}>{aiTest.message}</div>
              </div>
            )}
          </div>
        </div>

        {/* Platforms */}
        <div style={{ fontSize: 11, fontWeight: 700, color: p.text3, letterSpacing: 0.4, textTransform: 'uppercase', marginBottom: 12 }}>Platforms</div>
        <div style={{ fontSize: 12, color: p.text2, marginTop: -4, marginBottom: 16 }}>Choose which routes appear in the sidebar. Hidden platforms keep their symlinks intact — they just stay out of view.</div>
        {(['Core', 'Coding', 'Lobster'] as const).map((group) => (
          <div key={group} style={{ marginBottom: 24 }}>
            <div style={{ fontSize: 11, fontWeight: 700, color: p.text3, letterSpacing: 0.4, textTransform: 'uppercase', marginBottom: 8 }}>{group}</div>
            <div style={{ border: '1px solid ' + p.line, borderRadius: 8, overflow: 'hidden' }}>
              {platforms.filter((pl) => pl.group === group).map((pl, i, arr) => (
                <div key={pl.id} style={{
                  display: 'flex', alignItems: 'center', gap: 12, padding: '10px 14px',
                  borderBottom: i < arr.length - 1 ? '1px solid ' + p.lineSoft : 'none',
                  background: pl.isHub ? p.hubSoft : 'transparent',
                }}>
                  <div style={{ fontSize: 13, color: p.text, flex: 1, display: 'flex', alignItems: 'center', gap: 8 }}>
                    {pl.name}
                    {pl.isHub && <span style={{ fontSize: 9, color: p.hubText, background: '#fff', padding: '1px 5px', borderRadius: 3, fontWeight: 700 }}>HUB</span>}
                  </div>
                  <div style={{ fontSize: 11, color: p.text3, fontFamily: MONO }}>{pl.path}</div>
                  <Toggle on={!config.hiddenPlatforms.includes(pl.id)} disabled={pl.isHub}
                    onClick={() => !pl.isHub && toggleHidden(pl.id)} p={p} />
                </div>
              ))}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

// ─── Import modal ─────────────────────────────────────────────────
function ImportModal({ open, onClose, onAdd, pending, p }: {
  open: boolean; onClose: () => void; onAdd: (name: string, tagline: string) => void; pending: boolean; p: Palette;
}) {
  const [name, setName] = useState('');
  const [tagline, setTagline] = useState('');
  if (!open) return null;
  return (
    <div onClick={() => { if (!pending) onClose(); }} style={{
      position: 'absolute', inset: 0, background: 'rgba(0,0,0,0.35)',
      display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 10,
    }}>
      <div onClick={(e) => e.stopPropagation()} style={{
        width: 440, background: p.panel, borderRadius: 12, padding: 24, boxShadow: '0 20px 60px rgba(0,0,0,0.3)',
      }}>
        <div style={{ fontSize: 17, fontWeight: 600, color: p.text }}>Import new skill</div>
        <div style={{ fontSize: 12, color: p.text2, marginTop: 4 }}>Add a SKILL.md and friends to Central. Routes can be set after.</div>

        <div style={{ marginTop: 18 }}>
          <div style={{ fontSize: 11, color: p.text2, marginBottom: 4 }}>Skill name</div>
          <input value={name} disabled={pending} onChange={(e) => setName(e.target.value)} placeholder="e.g. csv-cleaner"
            style={{ width: '100%', boxSizing: 'border-box', border: '1px solid ' + p.line, borderRadius: 6, padding: '7px 10px', fontSize: 13, fontFamily: MONO, outline: 'none', color: p.text, background: p.panel }} />
        </div>
        <div style={{ marginTop: 12 }}>
          <div style={{ fontSize: 11, color: p.text2, marginBottom: 4 }}>One-line description</div>
          <input value={tagline} disabled={pending} onChange={(e) => setTagline(e.target.value)} placeholder="What does it do?"
            style={{ width: '100%', boxSizing: 'border-box', border: '1px solid ' + p.line, borderRadius: 6, padding: '7px 10px', fontSize: 13, fontFamily: FONT_SANS, outline: 'none', color: p.text, background: p.panel }} />
        </div>
        <div style={{ marginTop: 14, fontSize: 11, color: p.text3, fontFamily: MONO, padding: '8px 10px', background: 'rgba(0,0,0,0.04)', borderRadius: 6 }}>
          Will be written to ~/.skillloom/skills/{name || '…'}/
        </div>

        <div style={{ display: 'flex', gap: 8, marginTop: 20, justifyContent: 'flex-end' }}>
          <button onClick={onClose} disabled={pending} style={{
            border: '1px solid ' + p.line, background: p.panel, color: p.text, borderRadius: 6,
            padding: '6px 14px', fontSize: 13, cursor: pending ? 'not-allowed' : 'pointer', fontFamily: FONT_SANS,
            opacity: pending ? 0.6 : 1,
          }}>Cancel</button>
          <button onClick={() => { onAdd(name, tagline); if (!pending) { setName(''); setTagline(''); } }} disabled={!name || pending}
            style={{
              border: 0, background: p.accent, color: '#fff', borderRadius: 6,
              padding: '6px 14px', fontSize: 13, cursor: name && !pending ? 'pointer' : 'not-allowed',
              fontFamily: FONT_SANS, fontWeight: 500, opacity: name && !pending ? 1 : 0.5,
            }}>{pending ? 'Adding' : 'Add skill'}</button>
        </div>
      </div>
    </div>
  );
}

// ─── App ──────────────────────────────────────────────────────────
const DEFAULT_CONFIG: Config = {
  palette: 'cool', density: 'comfortable', view: 'list',
  hiddenPlatforms: ['cursor', 'gemini', 'copilot', 'windsurf', 'aider', 'qclaw', 'easyclaw', 'workbuddy'],
  aiProvider: 'anthropic',
  aiEndpoint: AI_PRESETS.anthropic.endpoint,
  aiModel: AI_PRESETS.anthropic.model,
};

export default function App() {
  useFontLink();

  const [platforms, setPlatforms] = useState<Platform[]>([]);
  const [skills, setSkills] = useState<Skill[]>([]);
  const [details, setDetails] = useState<Record<string, SkillDetail>>({});
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [summaries, setSummaries] = useState<Record<string, string>>({});
  const [summaryPendingId, setSummaryPendingId] = useState<string | null>(null);
  const [summaryErrors, setSummaryErrors] = useState<Record<string, string>>({});
  const [config, setConfigState] = useState<Config>(DEFAULT_CONFIG);
  const [loading, setLoading] = useState(true);
  const [bootError, setBootError] = useState<string | null>(null);
  const [notice, setNotice] = useState<AppNotice | null>(null);
  const [scanPending, setScanPending] = useState(false);
  const [routePending, setRoutePending] = useState<string | null>(null);
  const [deletePending, setDeletePending] = useState(false);
  const [importPending, setImportPending] = useState(false);
  const [apiKeyConfigured, setApiKeyConfigured] = useState(false);
  const [apiKeyPending, setApiKeyPending] = useState(false);
  const [aiTestPending, setAiTestPending] = useState(false);
  const [aiTest, setAiTest] = useState<AiTestState>(null);

  const [active, setActive] = useState('central');
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [q, setQ] = useState('');
  const [importOpen, setImportOpen] = useState(false);

  const p = PALETTES[config.palette];

  const showNotice = useCallback((message: string, tone: NoticeTone = 'error') => {
    setNotice({ id: Date.now(), tone, message });
  }, []);

  useEffect(() => {
    if (!notice) return;
    const timeout = window.setTimeout(() => setNotice(null), 5000);
    return () => window.clearTimeout(timeout);
  }, [notice]);

  const refreshSkills = useCallback(async () => {
    setScanPending(true);
    try {
      const next = await api.scanSkills();
      setSkills(next);
      setSelectedId((current) => {
        if (current && next.some((skill) => skill.id === current)) return current;
        return next[0]?.id ?? null;
      });
    }
    catch (e) {
      console.error('scanSkills failed', e);
      showNotice('Scan failed: ' + e);
    } finally {
      setScanPending(false);
    }
  }, [showNotice]);

  // Boot
  useEffect(() => {
    (async () => {
      try {
        const [pls, cfg, keyStatus] = await Promise.all([api.listPlatforms(), api.getConfig(), api.getApiKeyStatus()]);
        setPlatforms(pls);
        setConfigState(cfg);
        setApiKeyConfigured(keyStatus.configured);
        await refreshSkills();
      } catch (e) {
        console.error(e);
        setBootError(String(e));
      } finally {
        setLoading(false);
      }
    })();
  }, [refreshSkills]);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;

    listen('skills-changed', () => {
      refreshSkills();
    })
      .then((cleanup) => {
        if (cancelled) cleanup();
        else unlisten = cleanup;
      })
      .catch((e) => {
        console.error('skills-changed listener failed', e);
        showNotice('File watcher listener failed: ' + e);
      });

    return () => {
      cancelled = true;
      if (unlisten) unlisten();
    };
  }, [refreshSkills, showNotice]);

  const setConfig = useCallback(async (patch: Partial<Config>) => {
    setConfigState((c) => ({ ...c, ...patch }));
    try { const next = await api.updateConfig(patch); setConfigState(next); }
    catch (e) {
      console.error('updateConfig failed', e);
      showNotice('Settings save failed: ' + e);
    }
  }, [showNotice]);

  useEffect(() => {
    setAiTest(null);
  }, [config.aiProvider, config.aiEndpoint, config.aiModel]);

  const saveApiKey = useCallback(async (key: string) => {
    setApiKeyPending(true);
    try {
      const status = await api.setApiKey(key);
      setApiKeyConfigured(status.configured);
      setAiTest(null);
      showNotice('API key saved to Keychain.', 'info');
    } catch (e) {
      console.error('setApiKey failed', e);
      showNotice('API key save failed: ' + e);
    } finally {
      setApiKeyPending(false);
    }
  }, [showNotice]);

  const clearApiKey = useCallback(async () => {
    setApiKeyPending(true);
    try {
      const status = await api.clearApiKey();
      setApiKeyConfigured(status.configured);
      setAiTest(null);
      showNotice('API key cleared.', 'info');
    } catch (e) {
      console.error('clearApiKey failed', e);
      showNotice('API key clear failed: ' + e);
    } finally {
      setApiKeyPending(false);
    }
  }, [showNotice]);

  const testAiConfig = useCallback(async () => {
    if (aiTestPending) return;
    setAiTestPending(true);
    setAiTest(null);
    try {
      const result = await api.testAiConfig();
      setAiTest({
        tone: 'info',
        message: `${result.provider} / ${result.model}: ${result.response}`,
      });
      showNotice('AI connection test succeeded.', 'info');
    } catch (e) {
      console.error('testAiConfig failed', e);
      setAiTest({ tone: 'error', message: String(e) });
      showNotice('AI connection test failed: ' + e);
    } finally {
      setAiTestPending(false);
    }
  }, [aiTestPending, showNotice]);

  const visiblePlatforms = useMemo(
    () => platforms.filter((pl) => !config.hiddenPlatforms.includes(pl.id)),
    [platforms, config.hiddenPlatforms],
  );

  const counts = useMemo(() => {
    const c: Record<string, number> = {};
    visiblePlatforms.forEach((pl) => (c[pl.id] = 0));
    skills.forEach((s) => s.routes.forEach((r) => { if (c[r] != null) c[r]++; }));
    return c;
  }, [skills, visiblePlatforms]);

  const filtered = useMemo(() => skills.filter((s) => {
    if (active === 'settings') return false;
    if (!s.routes.includes(active)) return false;
    if (q && !(s.title.toLowerCase().includes(q.toLowerCase()) || s.tagline.toLowerCase().includes(q.toLowerCase()))) return false;
    return true;
  }), [active, q, skills]);

  const selected = filtered.find((s) => s.id === selectedId) || filtered[0] || null;
  const selectedDetail = selected ? details[selected.id] : undefined;
  const selectedSummary = selected ? summaries[selected.id] : undefined;
  const selectedSummaryLoading = selected ? summaryPendingId === selected.id : false;
  const selectedSummaryError = selected ? summaryErrors[selected.id] ?? null : null;

  const loadSummary = useCallback(async (id: string, force = false) => {
    setSummaryPendingId(id);
    setSummaryErrors((current) => {
      if (!(id in current)) return current;
      const next = { ...current };
      delete next[id];
      return next;
    });
    try {
      const summary = await api.generateSummary(id, force);
      setSummaries((current) => ({ ...current, [id]: summary }));
    } catch (e) {
      console.error('generateSummary failed', e);
      setSummaryErrors((current) => ({ ...current, [id]: String(e) }));
      if (force) showNotice('Summary generation failed: ' + e);
    } finally {
      setSummaryPendingId((current) => current === id ? null : current);
    }
  }, [showNotice]);

  useEffect(() => {
    const id = selected?.id;
    if (!id) {
      setDetailLoading(false);
      setDetailError(null);
      return;
    }
    if (details[id]) {
      setDetailLoading(false);
      setDetailError(null);
      return;
    }

    let cancelled = false;
    setDetailLoading(true);
    setDetailError(null);
    api.getSkillDetail(id)
      .then((detail) => {
        if (cancelled) return;
        setDetails((current) => ({ ...current, [id]: detail }));
      })
      .catch((e) => {
        if (cancelled) return;
        console.error('getSkillDetail failed', e);
        setDetailError(String(e));
      })
      .finally(() => {
        if (!cancelled) setDetailLoading(false);
      });

    return () => { cancelled = true; };
  }, [selected?.id, details]);

  useEffect(() => {
    const id = selected?.id;
    if (!id || summaries[id] != null) return;
    loadSummary(id, false);
  }, [selected?.id, summaries, loadSummary]);

  const toggleRoute = async (pid: string) => {
    if (!selected || routePending) return;
    const on = selected.routes.includes(pid);
    setRoutePending(pid);
    try {
      if (on) await api.removeRoute(selected.id, pid);
      else await api.addRoute(selected.id, pid);
      await refreshSkills();
    } catch (e) {
      showNotice((on ? 'Remove route failed: ' : 'Add route failed: ') + e);
    } finally {
      setRoutePending(null);
    }
  };

  const deleteSelected = async () => {
    if (!selected || deletePending) return;
    setDeletePending(true);
    try { await api.deleteSkill(selected.id); await refreshSkills(); }
    catch (e) { showNotice('Delete failed: ' + e); }
    finally { setDeletePending(false); }
  };

  const addSkill = async (name: string, tagline: string) => {
    if (importPending) return;
    setImportPending(true);
    try {
      const sk = await api.importSkill(name, tagline);
      await refreshSkills();
      setSelectedId(sk.id);
      setActive('central');
      setImportOpen(false);
    } catch (e) { showNotice('Import failed: ' + e); }
    finally { setImportPending(false); }
  };

  const activeTitle = active === 'central' ? 'Central Skills' :
    active === 'settings' ? 'Settings' :
    platforms.find((pl) => pl.id === active)?.name || active;

  if (bootError) {
    return (
      <div style={{ position: 'absolute', inset: 0, display: 'grid', placeItems: 'center', padding: 40, fontFamily: FONT_SANS, color: '#1d1d1f' }}>
        <div style={{ maxWidth: 520 }}>
          <div style={{ fontSize: 16, fontWeight: 600 }}>Failed to start</div>
          <pre style={{ marginTop: 12, fontSize: 12, background: '#f3f3f5', padding: 12, borderRadius: 6, whiteSpace: 'pre-wrap' }}>{bootError}</pre>
        </div>
      </div>
    );
  }

  if (loading) {
    return (
      <div data-tauri-drag-region style={{ position: 'absolute', inset: 0, background: p.bg, display: 'grid', placeItems: 'center', fontFamily: FONT_SANS, color: p.text3, fontSize: 13 }}>
        Loading…
      </div>
    );
  }

  return (
    <div style={{
      position: 'absolute', inset: 0,
      background: p.bg, fontFamily: FONT_SANS, color: p.text,
      display: 'flex', overflow: 'hidden',
    }}>
      <WindowDragStrip />
      <NoticeToast notice={notice} onClose={() => setNotice(null)} p={p} />
      <Sidebar active={active} setActive={setActive} counts={counts}
        visiblePlatforms={visiblePlatforms} totalSkills={skills.length} p={p} />

      {active === 'settings' ? (
        <SettingsPane p={p} platforms={platforms} config={config} setConfig={setConfig}
          apiKeyConfigured={apiKeyConfigured} apiKeyPending={apiKeyPending}
          aiTestPending={aiTestPending} aiTest={aiTest}
          onSaveApiKey={saveApiKey} onClearApiKey={clearApiKey} onTestAi={testAiConfig} />
      ) : (
        <>
          <div style={{ width: 360, borderRight: '1px solid ' + p.line, display: 'flex', flexDirection: 'column', background: p.panel, flexShrink: 0 }}>
            <div data-tauri-drag-region style={{ height: 28, flexShrink: 0, background: p.panel }} />
            <ListHeader title={activeTitle + ' · ' + filtered.length} q={q} setQ={setQ}
              view={config.view} setView={(v) => setConfig({ view: v })}
              onImport={() => setImportOpen(true)} onRefresh={refreshSkills}
              refreshing={scanPending} importDisabled={importPending}
              p={p} density={config.density} />
            <div style={{ flex: 1, overflowY: 'auto' }}>
              {filtered.length === 0 ? (
                <div style={{ padding: 40, textAlign: 'center', color: p.text3, fontSize: 12 }}>
                  {active === 'central'
                    ? <>No skills in <code style={{ fontFamily: MONO }}>~/.skillloom/skills/</code> yet.<br/>Click ＋ Import to add one.</>
                    : <>No skills routed here.<br/>Toggle this platform on a skill in Central.</>}
                </div>
              ) : config.view === 'list' ? (
                filtered.map((s) => (
                  <SkillRow key={s.id} skill={s} selected={selected?.id === s.id}
                    onClick={() => setSelectedId(s.id)} p={p} density={config.density} platforms={platforms} />
                ))
              ) : (
                <div style={{ padding: 12, display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8 }}>
                  {filtered.map((s) => (
                    <SkillCard key={s.id} skill={s} selected={selected?.id === s.id}
                      onClick={() => setSelectedId(s.id)} p={p} platforms={platforms} />
                  ))}
                </div>
              )}
            </div>
          </div>
          {selected ? (
            <div style={{ flex: 1, display: 'flex', flexDirection: 'column', minWidth: 0 }}>
              <div data-tauri-drag-region style={{ height: 28, flexShrink: 0, background: p.panel }} />
              <Detail skill={selected} detail={selectedDetail} detailLoading={detailLoading} detailError={detailError}
                summary={selectedSummary} summaryLoading={selectedSummaryLoading} summaryError={selectedSummaryError}
                onRegenerateSummary={() => loadSummary(selected.id, true)}
                toggleRoute={toggleRoute} onDelete={deleteSelected} onNotice={showNotice}
                routePending={routePending} deletePending={deletePending}
                visiblePlatforms={visiblePlatforms} p={p} paletteKey={config.palette} />
            </div>
          ) : (
            <div style={{ flex: 1, display: 'flex', flexDirection: 'column', minWidth: 0, background: p.panel }}>
              <div data-tauri-drag-region style={{ height: 28, flexShrink: 0 }} />
              <div style={{ flex: 1, display: 'grid', placeItems: 'center', color: p.text3, fontSize: 13 }}>
                {skills.length === 0 ? 'Import a skill to get started.' : 'Pick a skill from the list.'}
              </div>
            </div>
          )}
        </>
      )}

      <ImportModal open={importOpen} onClose={() => setImportOpen(false)} onAdd={addSkill} pending={importPending} p={p} />
    </div>
  );
}
