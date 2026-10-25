import { useState } from 'react';
import { PALETTES, PALETTE_KEYS, PALETTE_OPTIONS, type Palette } from './palettes';
import type { Config, Finding, Overview, SyncResult } from './types';
import { Button, DragSpacer, FONT_SANS, MONO, Segmented, agentName } from './ui';

const LEVEL_MARK: Record<Finding['level'], string> = { error: '✗', warn: '!', info: '·' };

function Heading({ children, p }: { children: string; p: Palette }) {
  return <div style={{ fontSize: 11, fontWeight: 700, color: p.text3, letterSpacing: 0.4, textTransform: 'uppercase', marginBottom: 12 }}>{children}</div>;
}

export function DoctorPane({ findings, loading, onRecheck, onSync, p }: {
  findings: Finding[]; loading: boolean; onRecheck: () => void;
  onSync: (dryRun: boolean) => Promise<SyncResult | null>; p: Palette;
}) {
  const [preview, setPreview] = useState<SyncResult | null>(null);
  const [busy, setBusy] = useState(false);
  const color = (l: Finding['level']) => (l === 'error' ? p.danger : l === 'warn' ? '#a05a00' : p.text3);
  const run = async (dryRun: boolean) => {
    setBusy(true);
    const r = await onSync(dryRun);
    setPreview(dryRun ? r : null);
    setBusy(false);
  };
  return (
    <div style={{ flex: 1, overflowY: 'auto', background: p.panel }}>
      <DragSpacer />
      <div style={{ padding: '4px 40px 32px' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <div style={{ fontSize: 22, fontWeight: 600, color: p.text, letterSpacing: -0.3, flex: 1 }}>Doctor</div>
          <Button onClick={onRecheck} disabled={loading || busy} p={p}>{loading ? '检查中…' : '重新检查'}</Button>
          <Button onClick={() => run(true)} disabled={busy} p={p}>预览同步</Button>
        </div>
        <div style={{ fontSize: 13, color: p.text2, marginTop: 4, marginBottom: 20 }}>
          检查断链、冲突、清单与发布区是否一致。同步只会改动指向 <code style={{ fontFamily: MONO }}>released/</code> 的链接。
        </div>

        {preview && (
          <div style={{ border: '1px solid ' + p.line, borderRadius: 8, padding: '12px 14px', marginBottom: 20 }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 8 }}>
              <div style={{ fontSize: 13, fontWeight: 600, color: p.text, flex: 1 }}>
                同步预览：{preview.changes.length ? `${preview.changes.length} 项变更` : '无需变更'}，保持 {preview.kept} 项
              </div>
              <Button onClick={() => setPreview(null)} p={p}>关闭</Button>
              {preview.changes.some((c) => c.action !== 'conflict') && (
                <Button primary onClick={() => run(false)} disabled={busy} p={p}>执行同步</Button>
              )}
            </div>
            {[...preview.changes.map((c) => `${c.action === 'conflict' ? '!' : c.action === 'removed' ? '−' : '+'} ${agentName(c.agent)} / ${c.name}：${c.message}`), ...preview.notes.map((n) => '· ' + n)].map((line, i) => (
              <div key={i} style={{ fontFamily: MONO, fontSize: 11, color: p.text2, lineHeight: 1.7 }}>{line}</div>
            ))}
          </div>
        )}

        {findings.length === 0 ? (
          <div style={{ fontSize: 13, color: p.hubText }}>{loading ? '检查中…' : '✓ 一切正常'}</div>
        ) : (
          <div style={{ border: '1px solid ' + p.line, borderRadius: 8, overflow: 'hidden' }}>
            {findings.map((f, i) => (
              <div key={i} style={{ display: 'flex', gap: 10, padding: '9px 14px', borderBottom: i < findings.length - 1 ? '1px solid ' + p.lineSoft : 'none', fontSize: 12, lineHeight: 1.5 }}>
                <span style={{ color: color(f.level), fontWeight: 700, width: 10, flexShrink: 0, fontFamily: MONO }}>{LEVEL_MARK[f.level]}</span>
                <span style={{ color: f.level === 'info' ? p.text2 : p.text, overflowWrap: 'anywhere' }}>{f.message}</span>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

export function SettingsPane({ p, config, setConfig, overview }: {
  p: Palette; config: Config; setConfig: (patch: Partial<Config>) => Promise<void>; overview: Overview | null;
}) {
  return (
    <div style={{ flex: 1, overflowY: 'auto', background: p.panel }}>
      <DragSpacer />
      <div style={{ padding: '4px 40px 32px' }}>
        <div style={{ fontSize: 22, fontWeight: 600, color: p.text, letterSpacing: -0.3 }}>Settings</div>
        <div style={{ fontSize: 13, color: p.text2, marginTop: 4, marginBottom: 24 }}>外观偏好保存在本机；skill 数据和分发规则在仓库的 skills.toml 里。</div>

        <div style={{ marginBottom: 32 }}>
          <Heading p={p}>Appearance</Heading>
          <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: 12 }}>
            {PALETTE_KEYS.map((key) => {
              const pal = PALETTES[key];
              const swatches = PALETTE_OPTIONS[PALETTE_KEYS.indexOf(key)];
              const selected = config.palette === key;
              return (
                <button key={key} onClick={() => setConfig({ palette: key })}
                  style={{
                    border: '1px solid ' + (selected ? pal.accent : p.line), background: pal.panel, borderRadius: 10, padding: 0,
                    cursor: 'pointer', textAlign: 'left', overflow: 'hidden', fontFamily: FONT_SANS,
                    boxShadow: selected ? '0 0 0 2px ' + pal.accentSoft : 'none', transition: 'box-shadow 0.15s, border-color 0.15s',
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

        <div style={{ marginBottom: 32 }}>
          <Heading p={p}>Preferences</Heading>
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

        {overview && (
          <div>
            <Heading p={p}>Repository</Heading>
            <div style={{ border: '1px solid ' + p.line, borderRadius: 8, overflow: 'hidden', fontSize: 12 }}>
              {[
                ['根目录', overview.root, `来源：${overview.rootSource}`],
                ['清单', overview.manifestPath, '编辑 [agents] 增减 agent，改完回到这里刷新'],
                ...overview.agents.map((a) => [agentName(a.id), a.path, a.installed ? '' : '未安装（目录不存在）']),
              ].map(([label, value, note], i, arr) => (
                <div key={i} style={{ display: 'grid', gridTemplateColumns: '110px 1fr', gap: 10, padding: '10px 14px', borderBottom: i < arr.length - 1 ? '1px solid ' + p.lineSoft : 'none' }}>
                  <div style={{ color: p.text, fontWeight: 500 }}>{label}</div>
                  <div>
                    <div style={{ fontFamily: MONO, fontSize: 11, color: p.text2, overflowWrap: 'anywhere' }}>{value}</div>
                    {note && <div style={{ fontSize: 11, color: p.text3, marginTop: 2 }}>{note}</div>}
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

export function NewSkillModal({ open, onClose, onCreate, pending, root, p }: {
  open: boolean; onClose: () => void; onCreate: (name: string) => void; pending: boolean; root: string; p: Palette;
}) {
  const [name, setName] = useState('');
  if (!open) return null;
  const valid = /^[a-z0-9][a-z0-9-]*$/.test(name);
  return (
    <div onClick={() => { if (!pending) onClose(); }} style={{ position: 'absolute', inset: 0, background: 'rgba(0,0,0,0.35)', display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 10 }}>
      <div onClick={(e) => e.stopPropagation()} style={{ width: 440, background: p.panel, borderRadius: 12, padding: 24, boxShadow: '0 20px 60px rgba(0,0,0,0.3)' }}>
        <div style={{ fontSize: 17, fontWeight: 600, color: p.text }}>New skill</div>
        <div style={{ fontSize: 12, color: p.text2, marginTop: 4, lineHeight: 1.5 }}>在开发区创建 SKILL.md 骨架。它不会被任何 agent 加载，测试满意后再发布。</div>
        <div style={{ marginTop: 18 }}>
          <div style={{ fontSize: 11, color: p.text2, marginBottom: 4 }}>Skill name</div>
          <input autoFocus value={name} disabled={pending} onChange={(e) => setName(e.target.value.trim())} placeholder="e.g. csv-cleaner"
            onKeyDown={(e) => { if (e.key === 'Enter' && valid && !pending) onCreate(name); }}
            style={{ width: '100%', boxSizing: 'border-box', border: '1px solid ' + (name && !valid ? p.danger : p.line), borderRadius: 6, padding: '7px 10px', fontSize: 13, fontFamily: MONO, outline: 'none', color: p.text, background: p.panel }} />
          {name && !valid && <div style={{ fontSize: 11, color: p.danger, marginTop: 4 }}>只能用小写字母、数字和连字符，且以字母或数字开头。</div>}
        </div>
        <div style={{ marginTop: 14, fontSize: 11, color: p.text3, fontFamily: MONO, padding: '8px 10px', background: 'rgba(0,0,0,0.04)', borderRadius: 6 }}>
          {root}/dev/{name || '…'}/SKILL.md
        </div>
        <div style={{ display: 'flex', gap: 8, marginTop: 20, justifyContent: 'flex-end' }}>
          <Button onClick={onClose} disabled={pending} p={p}>Cancel</Button>
          <Button primary onClick={() => { onCreate(name); setName(''); }} disabled={!valid || pending} p={p}>{pending ? 'Creating' : 'Create'}</Button>
        </div>
      </div>
    </div>
  );
}
