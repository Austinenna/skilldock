import { invoke } from '@tauri-apps/api/core';
import type { Platform, Skill, Config, SkillDetail, ApiKeyStatus } from './types';

export const api = {
  listPlatforms: () => invoke<Platform[]>('list_platforms'),
  scanSkills: () => invoke<Skill[]>('scan_skills'),
  getSkillDetail: (id: string) => invoke<SkillDetail>('get_skill_detail', { id }),
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
  getApiKeyStatus: () => invoke<ApiKeyStatus>('get_api_key_status'),
  setApiKey: (key: string) => invoke<ApiKeyStatus>('set_api_key', { key }),
  clearApiKey: () => invoke<ApiKeyStatus>('clear_api_key'),
};
