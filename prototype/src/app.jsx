// Skill Manager — interactive app.
// Layout: 3-pane Finder (Sidebar · List · Detail) on a macOS window.
// Theme + density + view mode live in Settings, persisted to localStorage.

(function () {
  const { useState, useMemo, useEffect, useCallback } = React;
  const { SKILLS, PLATFORMS } = window.SKILL_DATA;
  const { PALETTES, PALETTE_OPTIONS, PALETTE_KEYS } = window;

  const FONT_SANS = '-apple-system, BlinkMacSystemFont, "SF Pro Text", "Helvetica Neue", "Inter", sans-serif';
  const FONT_DISPLAY_WARM = '"Instrument Serif", Georgia, serif';
  const MONO = '"JetBrains Mono", "SF Mono", Menlo, monospace';

  // Inject display serif font once for the "warm" palette.
  if (!document.getElementById('proto-fonts')) {
    const l = document.createElement('link');
    l.id = 'proto-fonts';
    l.rel = 'stylesheet';
    l.href = 'https://fonts.googleapis.com/css2?family=Instrument+Serif:ital@0;1&family=Inter:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500&display=swap';
    document.head.appendChild(l);
  }

  // localStorage-backed state.
  function useLocalState(key, initial) {
    const storageKey = 'skillloom:' + key;
    const [v, setV] = useState(() => {
      try {
        const s = localStorage.getItem(storageKey);
        return s != null ? JSON.parse(s) : initial;
      } catch { return initial; }
    });
    useEffect(() => {
      try { localStorage.setItem(storageKey, JSON.stringify(v)); } catch {}
    }, [storageKey, v]);
    return [v, setV];
  }

  // ─────────────────────────────────────────────────────────────
  // Window chrome
  // ─────────────────────────────────────────────────────────────
  function TrafficLights() {
    return (
      <div style={{ display: 'flex', gap: 8, padding: '0 12px', height: 28, alignItems: 'center' }}>
        {['#ff5f57', '#febc2e', '#28c840'].map((c) => (
          <div key={c} style={{ width: 12, height: 12, borderRadius: 6, background: c, boxShadow: 'inset 0 0 0 .5px rgba(0,0,0,0.08)' }} />
        ))}
      </div>
    );
  }

  // ─────────────────────────────────────────────────────────────
  // Sidebar
  // ─────────────────────────────────────────────────────────────
  function SidebarSection({ title, p, children }) {
    return (
      <div style={{ padding: '14px 0 4px' }}>
        <div style={{ padding: '0 16px 6px', fontSize: 11, fontWeight: 700, color: p.text3, letterSpacing: 0.2, textTransform: 'uppercase' }}>{title}</div>
        {children}
      </div>
    );
  }

  function SidebarItem({ icon, label, count, active, accent, onClick, p }) {
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

  function Sidebar({ active, setActive, counts, visiblePlatforms, p, totalSkills }) {
    const platformAccents = { central: p.hub, claude: '#bf5af2', codex: '#5e9eff', openclaw: '#ff9f0a',
      cursor: '#a0a0a0', gemini: '#4285f4', copilot: '#1f2328', windsurf: '#2dd4bf', aider: '#dc2626',
      qclaw: '#ef4444', easyclaw: '#fbbf24', workbuddy: '#10b981' };
    const platformIcons = { central: '◎', claude: 'C', codex: 'X', openclaw: '🦞',
      cursor: 'c', gemini: 'G', copilot: 'p', windsurf: 'W', aider: 'A', qclaw: 'q', easyclaw: 'E', workbuddy: 'W' };

    const hub = visiblePlatforms.find((pl) => pl.isHub);
    const routes = visiblePlatforms.filter((pl) => !pl.isHub);

    return (
      <div style={{ width: 200, background: p.sidebar, borderRight: '1px solid ' + p.line, display: 'flex', flexDirection: 'column', flexShrink: 0 }}>
        <TrafficLights />
        <div style={{ flex: 1, overflowY: 'auto', paddingBottom: 12 }}>
          {hub && (
            <SidebarSection title="Hub" p={p}>
              <SidebarItem icon={platformIcons[hub.id]} label={hub.name} count={counts[hub.id]} accent={platformAccents[hub.id]}
                active={active === hub.id} onClick={() => setActive(hub.id)} p={p} />
            </SidebarSection>
          )}
          {routes.length > 0 && (
            <SidebarSection title="Routes" p={p}>
              {routes.map((pl) => (
                <SidebarItem key={pl.id} icon={platformIcons[pl.id] || pl.short[0]} label={pl.name} count={counts[pl.id]}
                  accent={platformAccents[pl.id] || p.text3}
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
          {totalSkills} skills · {visiblePlatforms.length - 1} routes
        </div>
      </div>
    );
  }

  // ─────────────────────────────────────────────────────────────
  // Middle: list header + rows + cards
  // ─────────────────────────────────────────────────────────────
  function ListHeader({ title, q, setQ, view, setView, onImport, p, density }) {
    return (
      <div style={{ borderBottom: '1px solid ' + p.line, padding: density === 'compact' ? '8px 12px' : '10px 12px 10px 14px', display: 'flex', flexDirection: 'column', gap: 8, background: p.panel }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <div style={{ fontSize: 13, fontWeight: 600, color: p.text }}>{title}</div>
          <div style={{ flex: 1 }} />
          <div style={{ display: 'flex', background: 'rgba(0,0,0,0.05)', borderRadius: 6, padding: 2 }}>
            {['list', 'grid'].map((m) => (
              <button key={m} onClick={() => setView(m)}
                style={{
                  border: 0, background: view === m ? p.panel : 'transparent', cursor: 'pointer',
                  borderRadius: 4, padding: '3px 8px', fontSize: 11, color: p.text, fontFamily: FONT_SANS,
                  boxShadow: view === m ? '0 1px 2px rgba(0,0,0,0.08)' : 'none',
                }}>{m === 'list' ? '☰' : '▦'}</button>
            ))}
          </div>
          <button onClick={onImport} style={{
            border: 0, background: p.accent, color: '#fff', cursor: 'pointer',
            borderRadius: 6, padding: '4px 10px', fontSize: 12, fontWeight: 500, fontFamily: FONT_SANS,
            display: 'flex', alignItems: 'center', gap: 4,
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

  function chipFor(routes, p) {
    return routes.slice(0, 4).map((r) => {
      const pl = PLATFORMS.find((x) => x.id === r);
      return (
        <span key={r} title={pl?.name}
          style={{
            fontSize: 9, padding: '1px 6px', borderRadius: 8,
            background: r === 'central' ? p.hubSoft : 'rgba(0,0,0,0.06)',
            color: r === 'central' ? p.hubText : p.text2,
            fontWeight: 500, letterSpacing: 0.2,
          }}>{pl?.short}</span>
      );
    });
  }

  function SkillRow({ skill, selected, onClick, p, density }) {
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
          width: 28, height: 28, borderRadius: 6,
          background: p.iconBg,
          color: p.text2, fontSize: 13, fontWeight: 700, fontFamily: MONO,
          display: 'grid', placeItems: 'center', flexShrink: 0, marginTop: 2,
        }}>{skill.title[0]}</div>
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ display: 'flex', alignItems: 'baseline', gap: 6 }}>
            <span style={{ fontSize: 13, fontWeight: 500, color: p.text, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{skill.title}</span>
            <span style={{ fontSize: 10, color: p.text3, fontFamily: MONO }}>{skill.version}</span>
          </div>
          <div style={{ fontSize: 12, color: p.text2, marginTop: 2, lineHeight: 1.4, overflow: 'hidden', textOverflow: 'ellipsis', display: '-webkit-box', WebkitLineClamp: 1, WebkitBoxOrient: 'vertical' }}>
            {skill.tagline}
          </div>
          {density !== 'compact' && (
            <div style={{ display: 'flex', gap: 3, marginTop: 6 }}>{chipFor(skill.routes, p)}</div>
          )}
        </div>
        <div style={{ width: 4, height: 4, borderRadius: 2, background: p.hub, marginTop: 8, flexShrink: 0 }} />
      </div>
    );
  }

  function SkillCard({ skill, selected, onClick, p }) {
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
        <div style={{ fontSize: 11, color: p.text2, lineHeight: 1.4, overflow: 'hidden', display: '-webkit-box', WebkitLineClamp: 2, WebkitBoxOrient: 'vertical' }}>{skill.tagline}</div>
        <div style={{ display: 'flex', gap: 3 }}>{chipFor(skill.routes, p)}</div>
        <div style={{ fontSize: 10, color: p.text3, fontFamily: MONO }}>{skill.routes.length} routes · {skill.size}</div>
      </div>
    );
  }

  // ─────────────────────────────────────────────────────────────
  // Detail pane
  // ─────────────────────────────────────────────────────────────
  function Section({ title, badge, open = true, setOpen, children, action, p }) {
    const collapsible = !!setOpen;
    return (
      <div style={{ borderBottom: '1px solid ' + p.lineSoft }}>
        <div onClick={collapsible ? () => setOpen(!open) : undefined}
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

  function Toggle({ on, disabled, onClick, p }) {
    return (
      <button onClick={onClick} disabled={disabled}
        style={{
          width: 34, height: 20, borderRadius: 10, border: 0, padding: 0,
          backgroundColor: on ? p.hub : 'rgba(120,120,128,0.32)',
          opacity: disabled ? 0.6 : 1, cursor: disabled ? 'not-allowed' : 'pointer',
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

  function Detail({ skill, toggleRoute, onDelete, visiblePlatforms, p, paletteKey, regenerating, onRegenerate }) {
    const [descOpen, setDescOpen] = useState(true);
    const [filesOpen, setFilesOpen] = useState(false);
    const [confirmDel, setConfirmDel] = useState(false);

    const display = paletteKey === 'warm' ? FONT_DISPLAY_WARM : FONT_SANS;
    const titleSize = paletteKey === 'warm' ? 28 : 19;
    const titleWeight = paletteKey === 'warm' ? 400 : 600;

    useEffect(() => { setConfirmDel(false); }, [skill.id]);

    return (
      <div style={{ flex: 1, overflowY: 'auto', background: p.panel }}>
        <div style={{ padding: '24px 28px 16px', borderBottom: '1px solid ' + p.lineSoft }}>
          <div style={{ display: 'flex', alignItems: 'flex-start', gap: 14 }}>
            <div style={{
              width: 52, height: 52, borderRadius: 12,
              background: p.iconLg,
              color: p.iconTextLg, fontSize: 22, fontWeight: 700, fontFamily: MONO,
              display: 'grid', placeItems: 'center',
              boxShadow: '0 4px 12px rgba(0,0,0,0.08)', flexShrink: 0,
            }}>{skill.title[0].toUpperCase()}</div>
            <div style={{ flex: 1, minWidth: 0 }}>
              <div style={{ fontSize: titleSize, fontWeight: titleWeight, color: p.text, letterSpacing: -0.3, fontFamily: display, lineHeight: 1.1 }}>{skill.title}</div>
              <div style={{ fontSize: 13, color: p.text2, marginTop: 4 }}>{skill.tagline}</div>
              <div style={{ display: 'flex', gap: 12, marginTop: 8, fontSize: 11, color: p.text3, fontFamily: MONO }}>
                <span>v{skill.version}</span>
                <span>·</span>
                <span>{skill.files} files</span>
                <span>·</span>
                <span>{skill.size}</span>
                <span>·</span>
                <span>updated {skill.updated}</span>
              </div>
            </div>
            <button onClick={() => {
              if (confirmDel) { onDelete(); setConfirmDel(false); }
              else setConfirmDel(true);
            }}
              style={{
                border: '1px solid ' + (confirmDel ? p.danger : p.line),
                background: confirmDel ? p.danger : p.panel,
                color: confirmDel ? '#fff' : p.danger, fontSize: 12, fontFamily: FONT_SANS,
                borderRadius: 6, padding: '5px 10px', cursor: 'pointer',
                transition: 'background-color 0.12s, color 0.12s',
              }}>{confirmDel ? 'Confirm delete' : 'Delete'}</button>
          </div>
        </div>

        <Section title="AI Summary" badge="generated · haiku-4-5" p={p} open={descOpen} setOpen={setDescOpen}
          action={
            <button onClick={(e) => { e.stopPropagation(); onRegenerate(); }} disabled={regenerating}
              style={{
                border: 0, background: 'transparent', color: p.accent, fontSize: 11, cursor: 'pointer',
                fontFamily: FONT_SANS, padding: '2px 6px', borderRadius: 4,
                opacity: regenerating ? 0.5 : 1,
              }}>↻ {regenerating ? 'Generating…' : 'Regenerate'}</button>
          }>
          <div style={{ padding: '4px 28px 20px' }}>
            <div style={{
              fontSize: 13, lineHeight: 1.6, color: p.text,
              padding: '14px 16px', borderRadius: 10,
              background: 'linear-gradient(180deg, ' + p.accentSoft + ', ' + p.accentSoft.replace(/0\.\d+/, '0.03') + ')',
              border: '1px solid ' + p.accentSoft,
              fontStyle: paletteKey === 'warm' ? 'italic' : 'normal',
              fontFamily: paletteKey === 'warm' ? FONT_DISPLAY_WARM : FONT_SANS,
              ...(paletteKey === 'warm' ? { fontSize: 16 } : {}),
            }}>{regenerating ? <span style={{ color: p.text3 }}>Reading SKILL.md and running the analyzer…</span> : skill.ai}</div>
            <div style={{ display: 'flex', gap: 6, marginTop: 10 }}>
              {skill.tags.map((t) => (
                <span key={t} style={{ fontSize: 11, padding: '2px 8px', borderRadius: 10, background: 'rgba(0,0,0,0.05)', color: p.text2, fontFamily: MONO }}>#{t}</span>
              ))}
            </div>
          </div>
        </Section>

        <Section title="Routing" badge={`${skill.routes.length} active`} p={p}>
          <div style={{ padding: '4px 28px 20px', display: 'flex', flexDirection: 'column', gap: 6 }}>
            {visiblePlatforms.map((pl) => {
              const on = skill.routes.includes(pl.id);
              return (
                <div key={pl.id} style={{
                  display: 'flex', alignItems: 'center', gap: 12,
                  padding: '10px 12px', border: '1px solid ' + p.line, borderRadius: 8,
                  background: on ? 'transparent' : 'rgba(0,0,0,0.015)',
                }}>
                  <div style={{ width: 24, height: 24, borderRadius: 6, background: pl.isHub ? p.hubSoft : 'rgba(0,0,0,0.06)', color: p.text2, fontSize: 12, fontWeight: 700, display: 'grid', placeItems: 'center', fontFamily: MONO, flexShrink: 0 }}>{pl.short[0]}</div>
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <div style={{ fontSize: 13, color: p.text, fontWeight: 500, display: 'flex', alignItems: 'center', gap: 6 }}>
                      {pl.name}
                      {pl.isHub && <span style={{ fontSize: 9, color: p.hubText, background: p.hubSoft, padding: '1px 5px', borderRadius: 3, fontWeight: 700 }}>HUB</span>}
                    </div>
                    <div style={{ fontSize: 11, color: p.text3, fontFamily: MONO, marginTop: 2, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{pl.path}{skill.title}</div>
                  </div>
                  <Toggle on={on} disabled={pl.isHub} onClick={() => !pl.isHub && toggleRoute(pl.id)} p={p} />
                </div>
              );
            })}
          </div>
        </Section>

        <Section title="Files" badge={String(skill.files)} p={p} open={filesOpen} setOpen={setFilesOpen}>
          <div style={{ padding: '4px 28px 24px', fontFamily: MONO, fontSize: 12, color: p.text2 }}>
            {['SKILL.md', 'handler.ts', 'schema.json', 'README.md', 'package.json'].slice(0, skill.files).map((f, i) => (
              <div key={f} style={{ padding: '6px 0', display: 'flex', justifyContent: 'space-between', borderBottom: i < Math.min(skill.files, 4) ? '1px solid ' + p.lineSoft : 'none' }}>
                <span>{f}</span>
                <span style={{ color: p.text3 }}>{[4, 12, 8, 6, 2][i] || 4} KB</span>
              </div>
            ))}
          </div>
        </Section>
      </div>
    );
  }

  // ─────────────────────────────────────────────────────────────
  // Segmented control (used in Settings preferences)
  // ─────────────────────────────────────────────────────────────
  function Segmented({ value, options, onChange, p }) {
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

  // ─────────────────────────────────────────────────────────────
  // Settings pane
  // ─────────────────────────────────────────────────────────────
  function SettingsPane({ p, visibleIds, setVisibleIds, paletteKey, setPaletteKey, density, setDensity, view, setView }) {
    return (
      <div style={{ flex: 1, overflowY: 'auto', background: p.panel, padding: '32px 40px' }}>
        <div style={{ fontSize: 22, fontWeight: 600, color: p.text, letterSpacing: -0.3 }}>Settings</div>
        <div style={{ fontSize: 13, color: p.text2, marginTop: 4, marginBottom: 24 }}>Manage appearance and which platforms appear in the sidebar.</div>

        {/* Appearance — theme picker */}
        <div style={{ marginBottom: 32 }}>
          <div style={{ fontSize: 11, fontWeight: 700, color: p.text3, letterSpacing: 0.4, textTransform: 'uppercase', marginBottom: 12 }}>Appearance</div>
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: 12 }}>
            {PALETTE_KEYS.map((key) => {
              const pal = PALETTES[key];
              const swatches = PALETTE_OPTIONS[PALETTE_KEYS.indexOf(key)];
              const selected = paletteKey === key;
              return (
                <button key={key} onClick={() => setPaletteKey(key)}
                  style={{
                    border: '1px solid ' + (selected ? pal.accent : p.line),
                    background: pal.panel, borderRadius: 10, padding: 0, cursor: 'pointer',
                    textAlign: 'left', overflow: 'hidden', fontFamily: FONT_SANS,
                    boxShadow: selected ? '0 0 0 2px ' + pal.accentSoft : 'none',
                    transition: 'box-shadow 0.15s, border-color 0.15s',
                  }}>
                  {/* preview chrome */}
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
                  {/* footer with name + swatches */}
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

        {/* Preferences — density + default view */}
        <div style={{ marginBottom: 32 }}>
          <div style={{ fontSize: 11, fontWeight: 700, color: p.text3, letterSpacing: 0.4, textTransform: 'uppercase', marginBottom: 12 }}>Preferences</div>
          <div style={{ border: '1px solid ' + p.line, borderRadius: 8, overflow: 'hidden' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '12px 14px', borderBottom: '1px solid ' + p.lineSoft }}>
              <div style={{ flex: 1, minWidth: 0 }}>
                <div style={{ fontSize: 13, color: p.text, fontWeight: 500 }}>List density</div>
                <div style={{ fontSize: 11, color: p.text3, marginTop: 2 }}>Tighter rows show more skills at once.</div>
              </div>
              <Segmented value={density} onChange={setDensity} p={p}
                options={[{ value: 'comfortable', label: 'Comfortable' }, { value: 'compact', label: 'Compact' }]} />
            </div>
            <div style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '12px 14px' }}>
              <div style={{ flex: 1, minWidth: 0 }}>
                <div style={{ fontSize: 13, color: p.text, fontWeight: 500 }}>Default view</div>
                <div style={{ fontSize: 11, color: p.text3, marginTop: 2 }}>Switchable any time from the list header.</div>
              </div>
              <Segmented value={view} onChange={setView} p={p}
                options={[{ value: 'list', label: 'List' }, { value: 'grid', label: 'Grid' }]} />
            </div>
          </div>
        </div>

        <div style={{ fontSize: 11, fontWeight: 700, color: p.text3, letterSpacing: 0.4, textTransform: 'uppercase', marginBottom: 12 }}>Platforms</div>
        <div style={{ fontSize: 12, color: p.text2, marginTop: -4, marginBottom: 16 }}>Choose which routes appear in the sidebar. Hidden platforms keep their symlinks intact — they just stay out of view.</div>
        {['Core', 'Coding', 'Lobster'].map((group) => (
          <div key={group} style={{ marginBottom: 24 }}>
            <div style={{ fontSize: 11, fontWeight: 700, color: p.text3, letterSpacing: 0.4, textTransform: 'uppercase', marginBottom: 8 }}>{group}</div>
            <div style={{ border: '1px solid ' + p.line, borderRadius: 8, overflow: 'hidden' }}>
              {PLATFORMS.filter((pl) => pl.group === group).map((pl, i, arr) => (
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
                  <Toggle on={visibleIds.includes(pl.id)} disabled={pl.isHub}
                    onClick={() => !pl.isHub && setVisibleIds((v) => v.includes(pl.id) ? v.filter((x) => x !== pl.id) : [...v, pl.id])} p={p} />
                </div>
              ))}
            </div>
          </div>
        ))}
      </div>
    );
  }

  // ─────────────────────────────────────────────────────────────
  // Import modal
  // ─────────────────────────────────────────────────────────────
  function ImportModal({ open, onClose, onAdd, p }) {
    const [name, setName] = useState('');
    const [tagline, setTagline] = useState('');
    if (!open) return null;
    return (
      <div onClick={onClose} style={{
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
            <input value={name} onChange={(e) => setName(e.target.value)} placeholder="e.g. csv-cleaner"
              style={{ width: '100%', boxSizing: 'border-box', border: '1px solid ' + p.line, borderRadius: 6, padding: '7px 10px', fontSize: 13, fontFamily: MONO, outline: 'none', color: p.text }} />
          </div>
          <div style={{ marginTop: 12 }}>
            <div style={{ fontSize: 11, color: p.text2, marginBottom: 4 }}>One-line description</div>
            <input value={tagline} onChange={(e) => setTagline(e.target.value)} placeholder="What does it do?"
              style={{ width: '100%', boxSizing: 'border-box', border: '1px solid ' + p.line, borderRadius: 6, padding: '7px 10px', fontSize: 13, fontFamily: FONT_SANS, outline: 'none', color: p.text }} />
          </div>
          <div style={{ marginTop: 14, fontSize: 11, color: p.text3, fontFamily: MONO, padding: '8px 10px', background: 'rgba(0,0,0,0.04)', borderRadius: 6 }}>
            Will be written to ~/.skillloom/skills/{name || '…'}/
          </div>

          <div style={{ display: 'flex', gap: 8, marginTop: 20, justifyContent: 'flex-end' }}>
            <button onClick={onClose} style={{
              border: '1px solid ' + p.line, background: p.panel, color: p.text, borderRadius: 6,
              padding: '6px 14px', fontSize: 13, cursor: 'pointer', fontFamily: FONT_SANS,
            }}>Cancel</button>
            <button onClick={() => { onAdd(name, tagline); setName(''); setTagline(''); }} disabled={!name}
              style={{
                border: 0, background: p.accent, color: '#fff', borderRadius: 6,
                padding: '6px 14px', fontSize: 13, cursor: name ? 'pointer' : 'not-allowed',
                fontFamily: FONT_SANS, fontWeight: 500, opacity: name ? 1 : 0.5,
              }}>Add skill</button>
          </div>
        </div>
      </div>
    );
  }

  // ─────────────────────────────────────────────────────────────
  // Main App
  // ─────────────────────────────────────────────────────────────
  function App() {
    const [paletteKey, setPaletteKey] = useLocalState('palette', 'cool');
    const [density, setDensity] = useLocalState('density', 'comfortable');
    const [view, setView] = useLocalState('view', 'list');
    const [visibleIds, setVisibleIds] = useLocalState('visibleIds', PLATFORMS.filter((pl) => pl.visible).map((pl) => pl.id));

    const p = PALETTES[paletteKey] || PALETTES.cool;

    const [active, setActive] = useState('central');
    const [selectedId, setSelectedId] = useState('pdf-extract');
    const [q, setQ] = useState('');
    const [importOpen, setImportOpen] = useState(false);
    const [regeneratingFor, setRegeneratingFor] = useState(null);

    const [skills, setSkills] = useState(SKILLS);
    const [routes, setRoutes] = useState(() => Object.fromEntries(SKILLS.map((s) => [s.id, [...s.routes]])));

    const visiblePlatforms = useMemo(() => PLATFORMS.filter((pl) => visibleIds.includes(pl.id)), [visibleIds]);

    const counts = useMemo(() => {
      const c = {};
      visiblePlatforms.forEach((pl) => c[pl.id] = 0);
      skills.forEach((s) => (routes[s.id] || []).forEach((r) => { if (c[r] != null) c[r]++; }));
      return c;
    }, [routes, skills, visiblePlatforms]);

    const filtered = useMemo(() => skills.filter((s) => {
      const r = routes[s.id] || [];
      if (active === 'settings') return false;
      if (!r.includes(active)) return false;
      if (q && !(s.title.toLowerCase().includes(q.toLowerCase()) || s.tagline.toLowerCase().includes(q.toLowerCase()))) return false;
      return true;
    }), [active, q, routes, skills]);

    // If selection isn't in filtered list, fall back to first.
    const selected = filtered.find((s) => s.id === selectedId) || filtered[0] || skills[0];
    const selectedWithRoutes = selected && { ...selected, routes: routes[selected.id] || [] };

    const toggleRoute = (pid) => setRoutes((r) => {
      const cur = r[selected.id] || [];
      const next = cur.includes(pid) ? cur.filter((x) => x !== pid) : [...cur, pid];
      return { ...r, [selected.id]: next };
    });

    const deleteSelected = () => {
      setSkills((s) => s.filter((x) => x.id !== selected.id));
      setRoutes((r) => { const { [selected.id]: _, ...rest } = r; return rest; });
    };

    const addSkill = (name, tagline) => {
      const id = name.trim().toLowerCase().replace(/\s+/g, '-');
      if (!id) return;
      const newSkill = {
        id, title: id,
        tagline: tagline || 'New skill (no description yet)',
        ai: 'No AI summary yet — click ↻ Regenerate to analyze SKILL.md.',
        version: '0.1.0', size: '4 KB', files: 1, updated: 'just now',
        tags: ['new'], routes: ['central'],
      };
      setSkills((s) => [newSkill, ...s]);
      setRoutes((r) => ({ ...r, [id]: ['central'] }));
      setSelectedId(id);
      setActive('central');
      setImportOpen(false);
    };

    const regenerate = () => {
      setRegeneratingFor(selected.id);
      setTimeout(() => {
        setSkills((arr) => arr.map((s) => s.id === selected.id ? {
          ...s, ai: s.ai.split('. ').reverse().join('. '),
        } : s));
        setRegeneratingFor(null);
      }, 1200);
    };

    const activeTitle = active === 'central' ? 'Central Skills' :
      active === 'settings' ? 'Settings' :
      PLATFORMS.find((pl) => pl.id === active)?.name || active;

    return (
      <div style={{
        position: 'absolute', inset: 0,
        display: 'flex', alignItems: 'center', justifyContent: 'center',
        padding: 32,
        background: 'radial-gradient(ellipse at 30% 20%, #d4d4dc 0%, #b8b8c4 60%, #9a9aa8 100%)',
      }}>
        <div style={{ width: 1240, maxWidth: '100%', height: '100%', maxHeight: 820, background: p.bg, fontFamily: FONT_SANS, color: p.text, display: 'flex', borderRadius: 10, overflow: 'hidden', boxShadow: '0 30px 80px rgba(0,0,0,0.25), 0 0 0 .5px rgba(0,0,0,0.15)', position: 'relative' }}>
          <Sidebar active={active} setActive={setActive} counts={counts} visiblePlatforms={visiblePlatforms} p={p} totalSkills={skills.length} />
          {active === 'settings' ? (
            <SettingsPane p={p} visibleIds={visibleIds} setVisibleIds={setVisibleIds}
              paletteKey={paletteKey} setPaletteKey={setPaletteKey}
              density={density} setDensity={setDensity}
              view={view} setView={setView} />
          ) : (
            <>
              <div style={{ width: 360, borderRight: '1px solid ' + p.line, display: 'flex', flexDirection: 'column', background: p.panel, flexShrink: 0 }}>
                <ListHeader title={activeTitle + ' · ' + filtered.length} q={q} setQ={setQ} view={view} setView={setView} onImport={() => setImportOpen(true)} p={p} density={density} />
                <div style={{ flex: 1, overflowY: 'auto' }}>
                  {filtered.length === 0 ? (
                    <div style={{ padding: 40, textAlign: 'center', color: p.text3, fontSize: 12 }}>No skills routed here.<br/>Use the toggles in the detail pane to add routes.</div>
                  ) : view === 'list' ? (
                    filtered.map((s) => (
                      <SkillRow key={s.id} skill={{ ...s, routes: routes[s.id] || [] }} selected={selected?.id === s.id} onClick={() => setSelectedId(s.id)} p={p} density={density} />
                    ))
                  ) : (
                    <div style={{ padding: 12, display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 8 }}>
                      {filtered.map((s) => (
                        <SkillCard key={s.id} skill={{ ...s, routes: routes[s.id] || [] }} selected={selected?.id === s.id} onClick={() => setSelectedId(s.id)} p={p} />
                      ))}
                    </div>
                  )}
                </div>
              </div>
              {selectedWithRoutes && <Detail skill={selectedWithRoutes} toggleRoute={toggleRoute} onDelete={deleteSelected} visiblePlatforms={visiblePlatforms} p={p} paletteKey={paletteKey} regenerating={regeneratingFor === selected.id} onRegenerate={regenerate} />}
            </>
          )}

          <ImportModal open={importOpen} onClose={() => setImportOpen(false)} onAdd={addSkill} p={p} />
        </div>
      </div>
    );
  }

  window.SkillManagerApp = App;
})();
