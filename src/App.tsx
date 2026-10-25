import { useCallback, useEffect, useMemo, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { api } from './ipc';
import { PALETTES } from './palettes';
import type { Config, Finding, Overview, SkillDetail, SyncResult } from './types';
import { Sidebar, type ViewKey } from './Sidebar';
import { ListHeader, SkillCard, SkillRow } from './SkillList';
import { Detail } from './Detail';
import { DoctorPane, NewSkillModal, SettingsPane } from './Panes';
import {
  DragSpacer, FONT_SANS, MONO, NoticeToast, STATE_LABELS, WindowDragStrip,
  agentName, describeSync, useFontLink, type AppNotice, type NoticeTone,
} from './ui';

const DEFAULT_CONFIG: Config = { palette: 'cool', density: 'comfortable', view: 'list' };

export default function App() {
  useFontLink();

  const [overview, setOverview] = useState<Overview | null>(null);
  const [details, setDetails] = useState<Record<string, SkillDetail>>({});
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailError, setDetailError] = useState<string | null>(null);
  const [findings, setFindings] = useState<Finding[]>([]);
  const [doctorLoading, setDoctorLoading] = useState(false);
  const [config, setConfigState] = useState<Config>(DEFAULT_CONFIG);
  const [loading, setLoading] = useState(true);
  const [bootError, setBootError] = useState<string | null>(null);
  const [notice, setNotice] = useState<AppNotice | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [pending, setPending] = useState<string | null>(null);
  const [newOpen, setNewOpen] = useState(false);
  const [newPending, setNewPending] = useState(false);

  const [active, setActive] = useState<ViewKey>('all');
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [gridDetailOpen, setGridDetailOpen] = useState(false);
  const [q, setQ] = useState('');

  const p = PALETTES[config.palette];
  const skills = overview?.skills ?? [];
  const agents = overview?.agents ?? [];

  const showNotice = useCallback((message: string, tone: NoticeTone = 'error') => {
    setNotice({ id: Date.now(), tone, message });
  }, []);

  useEffect(() => {
    if (!notice) return;
    const timeout = window.setTimeout(() => setNotice(null), notice.tone === 'error' ? 8000 : 5000);
    return () => window.clearTimeout(timeout);
  }, [notice]);

  const runDoctor = useCallback(async () => {
    setDoctorLoading(true);
    try { setFindings(await api.runDoctor()); }
    catch (e) { showNotice('Doctor failed: ' + e); }
    finally { setDoctorLoading(false); }
  }, [showNotice]);

  const refresh = useCallback(async () => {
    setRefreshing(true);
    try {
      const next = await api.getOverview();
      setOverview(next);
      setDetails({});
      setSelectedId((current) => (current && next.skills.some((s) => s.id === current) ? current : next.skills[0]?.id ?? null));
      void runDoctor();
    } catch (e) {
      console.error('getOverview failed', e);
      showNotice(String(e));
      throw e;
    } finally {
      setRefreshing(false);
    }
  }, [runDoctor, showNotice]);

  useEffect(() => {
    (async () => {
      try {
        setConfigState(await api.getConfig());
        await refresh();
      } catch (e) {
        setBootError(String(e));
      } finally {
        setLoading(false);
      }
    })();
  }, [refresh]);

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | null = null;
    listen('skills-changed', () => { refresh().catch(() => {}); })
      .then((cleanup) => { if (cancelled) cleanup(); else unlisten = cleanup; })
      .catch((e) => showNotice('File watcher listener failed: ' + e));
    return () => { cancelled = true; if (unlisten) unlisten(); };
  }, [refresh, showNotice]);

  const setConfig = useCallback(async (patch: Partial<Config>) => {
    setConfigState((c) => ({ ...c, ...patch }));
    try { setConfigState(await api.updateConfig(patch)); }
    catch (e) { showNotice('Settings save failed: ' + e); }
  }, [showNotice]);

  const counts = useMemo(() => {
    const c: Record<string, number> = { all: skills.length };
    for (const s of skills) {
      c['state:' + s.state] = (c['state:' + s.state] ?? 0) + 1;
      for (const a of s.agents) c['agent:' + a] = (c['agent:' + a] ?? 0) + 1;
    }
    return c;
  }, [skills]);

  const issueCount = findings.filter((f) => f.level !== 'info').length;

  const filtered = useMemo(() => skills.filter((s) => {
    if (active.startsWith('state:') && s.state !== active.slice(6)) return false;
    if (active.startsWith('agent:') && !s.agents.includes(active.slice(6))) return false;
    if (q && !(s.id.includes(q.toLowerCase()) || s.description.toLowerCase().includes(q.toLowerCase()))) return false;
    return true;
  }), [active, q, skills]);

  const selected = filtered.find((s) => s.id === selectedId) || filtered[0] || null;

  useEffect(() => {
    const id = selected?.id;
    if (!id || details[id]) { setDetailLoading(false); setDetailError(null); return; }
    let cancelled = false;
    setDetailLoading(true);
    setDetailError(null);
    api.getSkillDetail(id)
      .then((d) => { if (!cancelled) setDetails((cur) => ({ ...cur, [id]: d })); })
      .catch((e) => { if (!cancelled) setDetailError(String(e)); })
      .finally(() => { if (!cancelled) setDetailLoading(false); });
    return () => { cancelled = true; };
  }, [selected?.id, details]);

  useEffect(() => {
    if (config.view !== 'grid' || active === 'settings' || active === 'doctor') setGridDetailOpen(false);
  }, [active, config.view]);

  useEffect(() => {
    if (!gridDetailOpen) return;
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') setGridDetailOpen(false); };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [gridDetailOpen]);

  /** Run a mutating action, report its sync result, then refresh. */
  const mutate = async (key: string, label: string, action: () => Promise<SyncResult>) => {
    if (pending) return;
    setPending(key);
    try {
      const r = await action();
      const { text, hasConflict } = describeSync(r);
      showNotice(`${label}\n${text}`, hasConflict ? 'error' : 'info');
    } catch (e) {
      showNotice(`${label}失败：${e}`);
    } finally {
      setPending(null);
      await refresh().catch(() => {});
    }
  };

  const createSkill = async (name: string) => {
    setNewPending(true);
    try {
      const path = await api.newSkill(name);
      setNewOpen(false);
      setActive('all');
      await refresh();
      setSelectedId(name);
      showNotice(`已创建 ${path}/SKILL.md`, 'info');
    } catch (e) { showNotice('创建失败：' + e); }
    finally { setNewPending(false); }
  };

  const deleteSkill = async (name: string) => {
    if (pending) return;
    setPending('delete');
    try {
      const moved = await api.deleteSkill(name);
      setGridDetailOpen(false);
      showNotice(moved ? `已删除 ${name}\n开发区已移到 ${moved}` : `已删除 ${name}`, 'info');
    } catch (e) { showNotice('删除失败：' + e); }
    finally {
      setPending(null);
      await refresh().catch(() => {});
    }
  };

  const syncFromDoctor = async (dryRun: boolean) => {
    try {
      const r = await api.runSync(dryRun);
      if (!dryRun) {
        showNotice('同步完成\n' + describeSync(r).text, 'info');
        await refresh();
      }
      return r;
    } catch (e) { showNotice('同步失败：' + e); return null; }
  };

  const activeTitle = active === 'all' ? '全部'
    : active.startsWith('state:') ? STATE_LABELS[active.slice(6) as keyof typeof STATE_LABELS]
      : active.startsWith('agent:') ? agentName(active.slice(6)) : active;

  if (bootError) {
    return (
      <div style={{ position: 'absolute', inset: 0, display: 'grid', placeItems: 'center', padding: 40, fontFamily: FONT_SANS, color: '#1d1d1f' }}>
        <div style={{ maxWidth: 560 }}>
          <div style={{ fontSize: 16, fontWeight: 600 }}>无法读取 skill 仓库</div>
          <pre style={{ marginTop: 12, fontSize: 12, background: '#f3f3f5', padding: 12, borderRadius: 6, whiteSpace: 'pre-wrap' }}>{bootError}</pre>
          <div style={{ marginTop: 12, fontSize: 12, color: '#6e6e73', lineHeight: 1.6 }}>
            在终端运行 <code style={{ fontFamily: MONO }}>skl init</code>（或 <code style={{ fontFamily: MONO }}>skl init --root &lt;路径&gt;</code>）创建仓库后重新打开。
          </div>
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

  const detailFor = (onClose?: () => void) => selected && (
    <Detail skill={selected} detail={details[selected.id]} detailLoading={detailLoading} detailError={detailError}
      agents={agents} pending={pending} p={p} paletteKey={config.palette} onClose={onClose}
      onPublish={(to) => mutate('publish', `已发布 ${selected.id}`, () => api.publishSkill(selected.id, to))}
      onSetAgents={(to) => mutate('agents', `已更新 ${selected.id} 的分发`, () => api.setSkillAgents(selected.id, to))}
      onUnpublish={() => mutate('unpublish', `已取消发布 ${selected.id}`, () => api.unpublishSkill(selected.id))}
      onDelete={() => deleteSkill(selected.id)} />
  );

  return (
    <div style={{ position: 'absolute', inset: 0, background: p.bg, fontFamily: FONT_SANS, color: p.text, display: 'flex', overflow: 'hidden' }}>
      <WindowDragStrip />
      <NoticeToast notice={notice} onClose={() => setNotice(null)} p={p} />
      <Sidebar active={active} setActive={setActive} counts={counts} agents={agents}
        issueCount={issueCount} root={overview?.root ?? ''} p={p} />

      {active === 'settings' ? (
        <SettingsPane p={p} config={config} setConfig={setConfig} overview={overview} />
      ) : active === 'doctor' ? (
        <DoctorPane findings={findings} loading={doctorLoading} onRecheck={runDoctor} onSync={syncFromDoctor} p={p} />
      ) : (
        <>
          <div style={{
            width: config.view === 'grid' ? undefined : 360, flex: config.view === 'grid' ? 1 : undefined,
            borderRight: config.view === 'grid' ? 0 : '1px solid ' + p.line,
            display: 'flex', flexDirection: 'column', background: p.panel, minWidth: 0,
          }}>
            <DragSpacer bg={p.panel} />
            <ListHeader title={activeTitle + ' · ' + filtered.length} q={q} setQ={setQ}
              view={config.view} setView={(v) => { void setConfig({ view: v }); if (v !== 'grid') setGridDetailOpen(false); }}
              onNew={() => setNewOpen(true)} onRefresh={() => { refresh().catch(() => {}); }}
              refreshing={refreshing} p={p} density={config.density} />
            <div style={{ flex: 1, overflowY: 'auto' }}>
              {filtered.length === 0 ? (
                <div style={{ padding: 40, textAlign: 'center', color: p.text3, fontSize: 12, lineHeight: 1.6 }}>
                  {skills.length === 0
                    ? <>仓库里还没有 skill。<br />点 ＋ New 在开发区新建一个。</>
                    : '这里没有 skill。'}
                </div>
              ) : config.view === 'list' ? (
                filtered.map((s) => (
                  <SkillRow key={s.id} skill={s} selected={selected?.id === s.id} onClick={() => setSelectedId(s.id)} p={p} density={config.density} />
                ))
              ) : (
                <div style={{ padding: 16, display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(230px, 1fr))', gap: 10 }}>
                  {filtered.map((s) => (
                    <SkillCard key={s.id} skill={s} selected={selected?.id === s.id}
                      onClick={() => { setSelectedId(s.id); setGridDetailOpen(true); }} p={p} />
                  ))}
                </div>
              )}
            </div>
          </div>
          {config.view !== 'grid' && (
            <div style={{ flex: 1, display: 'flex', flexDirection: 'column', minWidth: 0, background: p.panel }}>
              <DragSpacer bg={p.panel} />
              {selected ? detailFor() : (
                <div style={{ flex: 1, display: 'grid', placeItems: 'center', color: p.text3, fontSize: 13 }}>Pick a skill from the list.</div>
              )}
            </div>
          )}
        </>
      )}

      {config.view === 'grid' && gridDetailOpen && selected && (
        <div role="dialog" aria-modal="true" aria-label={`${selected.id} skill details`}
          onMouseDown={(e) => { if (e.target === e.currentTarget) setGridDetailOpen(false); }}
          style={{ position: 'absolute', inset: 0, zIndex: 30, display: 'grid', placeItems: 'center', padding: '44px 32px 24px', background: 'rgba(20, 24, 32, 0.22)', backdropFilter: 'blur(2px)' }}>
          <div onMouseDown={(e) => e.stopPropagation()}
            style={{
              width: 'min(820px, calc(100vw - 64px))', height: 'min(860px, calc(100vh - 72px))', maxHeight: 'calc(100vh - 72px)',
              display: 'flex', flexDirection: 'column', overflow: 'hidden', background: p.panel, border: '1px solid ' + p.line,
              borderRadius: 16, boxShadow: '0 24px 80px rgba(0,0,0,0.28)',
            }}>
            {detailFor(() => setGridDetailOpen(false))}
          </div>
        </div>
      )}

      <NewSkillModal open={newOpen} onClose={() => setNewOpen(false)} onCreate={createSkill} pending={newPending} root={overview?.root ?? ''} p={p} />
    </div>
  );
}
