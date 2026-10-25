import { useEffect, useState } from 'react';
import { DiffSection } from './Diff';
import type { Palette } from './palettes';
import type { Agent, PaletteKey, Skill, SkillDetail } from './types';
import { AGENT_ACCENTS, Button, FONT_DISPLAY_WARM, FONT_SANS, MONO, Section, StateBadge, Toggle, agentName, useConfirm } from './ui';

/** pending: 'publish' | 'unpublish' | 'agents' | null */
export function Detail({ skill, detail, detailLoading, detailError, agents, pending, onPublish, onSetAgents, onUnpublish, onDelete, onClose, p, paletteKey }: {
  skill: Skill; detail?: SkillDetail; detailLoading: boolean; detailError: string | null;
  agents: Agent[]; pending: string | null;
  onPublish: (agents: string[] | null) => void; onSetAgents: (agents: string[]) => void; onUnpublish: () => void; onDelete: () => void;
  onClose?: () => void; p: Palette; paletteKey: PaletteKey;
}) {
  const [mdOpen, setMdOpen] = useState(false);
  const [filesOpen, setFilesOpen] = useState(true);
  // First publish defaults to every agent (same as `skl publish` without --to).
  const initialDraft = () => (skill.hasReleased || skill.agents.length ? skill.agents : agents.map((a) => a.id));
  const [draft, setDraft] = useState<string[]>(initialDraft);
  const publishConfirm = useConfirm(skill.id + skill.state);
  const unpublishConfirm = useConfirm(skill.id + skill.state);
  const deleteConfirm = useConfirm(skill.id);
  useEffect(() => { setDraft(initialDraft()); }, [skill.id, skill.agents.join(','), agents.length]);

  const display = paletteKey === 'warm' ? FONT_DISPLAY_WARM : FONT_SANS;
  const busy = pending !== null;
  // Before the first publish, agent toggles only pick where the skill will go.
  const live = skill.hasReleased;
  const shown = live ? skill.agents : draft;

  const toggleAgent = (id: string) => {
    const next = shown.includes(id) ? shown.filter((a) => a !== id) : [...shown, id];
    if (live) onSetAgents(next);
    else setDraft(next);
  };

  const publishLabel = pending === 'publish' ? '发布中…'
    : publishConfirm.armed ? '确认发布'
      : skill.state === 'devAhead' ? '发布更新'
        : skill.state === 'released' ? '已是最新' : '发布';
  const canPublish = skill.hasDev && skill.state !== 'released' && !busy;

  return (
    <div style={{ flex: 1, overflowY: 'auto', background: p.panel }}>
      <div style={{ padding: '24px 28px 16px', borderBottom: '1px solid ' + p.lineSoft }}>
        <div style={{ display: 'flex', alignItems: 'flex-start', gap: 14 }}>
          <div style={{
            width: 52, height: 52, borderRadius: 12, background: p.iconLg, color: p.iconTextLg,
            fontSize: 22, fontWeight: 700, fontFamily: MONO, display: 'grid', placeItems: 'center',
            boxShadow: '0 4px 12px rgba(0,0,0,0.08)', flexShrink: 0,
          }}>{skill.id[0].toUpperCase()}</div>
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8, flexWrap: 'wrap' }}>
              <div style={{ fontSize: paletteKey === 'warm' ? 28 : 19, fontWeight: paletteKey === 'warm' ? 400 : 600, color: p.text, letterSpacing: -0.3, fontFamily: display, lineHeight: 1.1 }}>{skill.id}</div>
              <StateBadge state={skill.state} p={p} />
            </div>
            <div style={{ fontSize: 13, color: p.text2, marginTop: 6, lineHeight: 1.5 }}>
              {skill.description || <span style={{ color: p.text3, fontStyle: 'italic' }}>No description</span>}
            </div>
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8, flexShrink: 0 }}>
            {skill.hasDev && (
              <Button primary={!publishConfirm.armed} danger={publishConfirm.armed} disabled={!canPublish}
                title={skill.state === 'released' ? '开发版与发布版一致' : '把 dev/ 复制为发布版并同步链接'}
                onClick={() => publishConfirm.run(() => onPublish(live ? null : draft))} p={p}>{publishLabel}</Button>
            )}
            {onClose && (
              <button type="button" onClick={onClose} aria-label="Close skill details"
                style={{ width: 28, height: 28, borderRadius: 14, border: '1px solid ' + p.line, background: p.panel, color: p.text2, fontSize: 20, lineHeight: 1, cursor: 'pointer', fontFamily: FONT_SANS }}>×</button>
            )}
          </div>
        </div>
        {publishConfirm.armed && (
          <div style={{ marginTop: 12, fontSize: 12, color: p.text2, lineHeight: 1.5 }}>
            {live
              ? <>将用 <code style={{ fontFamily: MONO }}>dev/{skill.id}</code> 覆盖发布版，{skill.agents.length ? skill.agents.map(agentName).join('、') : '没有 agent'} 会立即用上新版本。</>
              : <>将把 <code style={{ fontFamily: MONO }}>dev/{skill.id}</code> 复制为发布版，并链接给：{draft.length ? draft.map(agentName).join('、') : '（暂不分发）'}。</>}
            再点一次确认。
          </div>
        )}
      </div>

      {skill.state === 'devAhead' && <DiffSection skillId={skill.id} refreshKey={detail} p={p} />}

      <Section title="Agents" badge={live ? `${skill.agents.length} linked` : '发布时生效'} p={p}>
        <div style={{ padding: '4px 28px 20px', display: 'flex', flexDirection: 'column', gap: 6 }}>
          {!live && !skill.hasDev && (
            <div style={{ fontSize: 12, color: p.danger }}>清单里有这个 skill，但开发区和发布区都没有。</div>
          )}
          {agents.map((a) => {
            const on = shown.includes(a.id);
            const disabled = busy || (!live && !skill.hasDev);
            return (
              <div key={a.id} onClick={() => !disabled && toggleAgent(a.id)}
                style={{
                  display: 'flex', alignItems: 'center', gap: 12, padding: '10px 12px',
                  border: '1px solid ' + p.line, borderRadius: 8, opacity: a.installed ? 1 : 0.6,
                  background: on ? 'transparent' : 'rgba(0,0,0,0.015)', cursor: disabled ? 'not-allowed' : 'pointer',
                }}>
                <div style={{ width: 24, height: 24, borderRadius: 6, background: 'rgba(0,0,0,0.06)', color: AGENT_ACCENTS[a.id] ?? p.text2, fontSize: 12, fontWeight: 700, display: 'grid', placeItems: 'center', fontFamily: MONO, flexShrink: 0 }}>{agentName(a.id)[0]}</div>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontSize: 13, color: p.text, fontWeight: 500 }}>
                    {agentName(a.id)}{!a.installed && <span style={{ fontSize: 10, color: p.text3, marginLeft: 6 }}>未安装</span>}
                  </div>
                  <div style={{ fontSize: 11, color: p.text3, fontFamily: MONO, marginTop: 2, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{a.path}/{skill.id}</div>
                </div>
                <Toggle on={on} disabled={disabled} onClick={() => toggleAgent(a.id)} p={p} />
              </div>
            );
          })}
        </div>
      </Section>

      <Section title="Location" p={p}>
        <div style={{ padding: '4px 28px 18px', fontFamily: MONO, fontSize: 11, color: p.text2, display: 'grid', gridTemplateColumns: '64px 1fr', rowGap: 6 }}>
          <span style={{ color: p.text3 }}>dev</span><span style={{ overflowWrap: 'anywhere' }}>{detail?.devPath ?? '—'}</span>
          <span style={{ color: p.text3 }}>released</span><span style={{ overflowWrap: 'anywhere' }}>{detail?.releasedPath ?? '—（未发布）'}</span>
        </div>
      </Section>

      <Section title="SKILL.md" p={p} open={mdOpen} setOpen={setMdOpen}>
        <pre style={{ margin: '4px 28px 20px', padding: '12px 14px', borderRadius: 8, background: 'rgba(0,0,0,0.035)', fontFamily: MONO, fontSize: 11, lineHeight: 1.55, color: p.text, whiteSpace: 'pre-wrap', overflowWrap: 'anywhere', maxHeight: 420, overflowY: 'auto' }}>
          {detailLoading ? 'Loading…' : detail?.skillMd || '（没有 SKILL.md）'}
        </pre>
      </Section>

      <Section title="Files" badge={detail ? String(detail.files.length) : undefined} p={p} open={filesOpen} setOpen={setFilesOpen}>
        <div style={{ padding: '4px 28px 24px', fontFamily: MONO, fontSize: 12, color: p.text2 }}>
          {detailLoading ? (
            <div style={{ color: p.text3 }}>Loading file details…</div>
          ) : detailError ? (
            <div style={{ color: p.danger }}>{detailError}</div>
          ) : detail && detail.files.length > 0 ? (
            <div style={{ border: '1px solid ' + p.line, borderRadius: 8, overflow: 'hidden' }}>
              {detail.files.map((file, index) => (
                <div key={file.name} style={{
                  display: 'grid', gridTemplateColumns: 'minmax(0, 1fr) 78px 70px 92px', gap: 10, alignItems: 'center', padding: '8px 10px',
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
          ) : (
            <div style={{ color: p.text3 }}>Empty skill directory.</div>
          )}
        </div>
      </Section>

      <div style={{ padding: '18px 28px 28px', display: 'flex', flexDirection: 'column', gap: 12 }}>
        {skill.hasReleased && (
          <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
            <div style={{ flex: 1, fontSize: 11, color: p.text3, lineHeight: 1.5 }}>
              取消发布会删除所有 agent 里的链接和 <code style={{ fontFamily: MONO }}>released/{skill.id}</code>，开发区保留。
            </div>
            <Button danger={unpublishConfirm.armed} disabled={busy} onClick={() => unpublishConfirm.run(onUnpublish)} p={p}>
              {pending === 'unpublish' ? '处理中…' : unpublishConfirm.armed ? '确认取消发布' : '取消发布'}
            </Button>
          </div>
        )}
        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
          <div style={{ flex: 1, fontSize: 11, color: p.text3, lineHeight: 1.5 }}>
            删除会{skill.hasReleased ? '先取消发布，再' : ''}把 <code style={{ fontFamily: MONO }}>dev/{skill.id}</code> 移到仓库的 <code style={{ fontFamily: MONO }}>.trash/</code>，需要时可以移回来。
          </div>
          <Button danger disabled={busy} onClick={() => deleteConfirm.run(onDelete)} p={p}>
            {pending === 'delete' ? '删除中…' : deleteConfirm.armed ? '确认删除' : '删除 skill'}
          </Button>
        </div>
      </div>
    </div>
  );
}
