import { invoke } from '@tauri-apps/api/core';
import type { Config, Finding, Overview, SkillDetail, SyncResult } from './types';

export const api = {
  getOverview: () => invoke<Overview>('get_overview'),
  getSkillDetail: (id: string) => invoke<SkillDetail>('get_skill_detail', { id }),
  newSkill: (name: string) => invoke<string>('new_skill', { name }),
  /** agents: null keeps the agents already listed in skills.toml */
  publishSkill: (name: string, agents: string[] | null) =>
    invoke<SyncResult>('publish_skill', { name, agents }),
  setSkillAgents: (name: string, agents: string[]) =>
    invoke<SyncResult>('set_skill_agents', { name, agents }),
  unpublishSkill: (name: string) => invoke<SyncResult>('unpublish_skill', { name }),
  /** returns where dev/<name> was moved, or null */
  deleteSkill: (name: string) => invoke<string | null>('delete_skill', { name }),
  runSync: (dryRun: boolean) => invoke<SyncResult>('run_sync', { dryRun }),
  runDoctor: () => invoke<Finding[]>('run_doctor'),
  getConfig: () => invoke<Config>('get_config'),
  updateConfig: (patch: Partial<Config>) => invoke<Config>('update_config', { patch }),
};
