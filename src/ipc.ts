import { invoke } from '@tauri-apps/api/core';
import type {
  Platform,
  Skill,
  Config,
  SkillDetail,
  AiTestResult,
  InitPreview,
  InitResult,
} from './types';

export const api = {
  listPlatforms: () => invoke<Platform[]>('list_platforms'),
  previewRepositoryInit: () => invoke<InitPreview>('preview_repository_init'),
  runRepositoryInit: (selectedKeys: string[]) =>
    invoke<InitResult>('run_repository_init', { selectedKeys }),
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
  testAiConfig: (apiKey: string) => invoke<AiTestResult>('test_ai_config', { apiKey }),
  generateSummary: (skillId: string, force = false, apiKey?: string) =>
    invoke<string>('generate_summary', { skillId, force, apiKey }),
};
