import { invoke } from '@tauri-apps/api/core';
import type { Platform, Skill, Config } from './types';

export const api = {
  listPlatforms: () => invoke<Platform[]>('list_platforms'),
  scanSkills: () => invoke<Skill[]>('scan_skills'),
  addRoute: (skillId: string, platformId: string) =>
    invoke<void>('add_route', { skillId, platformId }),
  removeRoute: (skillId: string, platformId: string) =>
    invoke<void>('remove_route', { skillId, platformId }),
  importSkill: (name: string, tagline: string) =>
    invoke<Skill>('import_skill', { name, tagline }),
  deleteSkill: (id: string) => invoke<void>('delete_skill', { id }),
  getConfig: () => invoke<Config>('get_config'),
  updateConfig: (patch: Partial<Config>) =>
    invoke<Config>('update_config', { patch }),
};
