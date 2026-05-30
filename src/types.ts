export type PaletteKey = 'cool' | 'warm' | 'slate';
export type Density = 'compact' | 'comfortable';
export type View = 'list' | 'grid';
export type PlatformGroup = 'Core' | 'Coding' | 'Lobster';
export type AiProvider = 'anthropic' | 'chat';

export interface Platform {
  id: string;
  name: string;
  short: string;
  path: string;
  group: PlatformGroup;
  isHub: boolean;
}

export interface Skill {
  id: string;
  title: string;
  tagline: string;
  version: string;
  size: string;
  files: number;
  updated: string;
  tags: string[];
  routes: string[];
  routeConflicts: RouteConflict[];
}

export interface SkillFile {
  name: string;
  kind: string;
  size: string;
  modified: string;
}

export interface SkillDetail {
  skill: Skill;
  skillMd: string;
  sourcePath: string;
  files: SkillFile[];
}

export interface RouteConflict {
  platformId: string;
  message: string;
}

export interface Config {
  palette: PaletteKey;
  density: Density;
  view: View;
  hiddenPlatforms: string[];
  aiProvider: AiProvider;
  aiEndpoint: string;
  aiModel: string;
}

export interface AiTestResult {
  provider: AiProvider;
  model: string;
  response: string;
}

export type InitAction =
  | 'migrateToCentral'
  | 'linkExistingCentral'
  | 'linkPlannedCentral'
  | 'resolveConflictSource'
  | 'alreadyRouted'
  | 'skipConflict'
  | 'skipInvalid';

export interface InitPreviewItem {
  key: string;
  id: string;
  title: string;
  platformId: string;
  platformName: string;
  platformPath: string;
  sourcePath: string;
  contentPath: string;
  sourceIsSymlink: boolean;
  targetId: string;
  targetPath: string;
  action: InitAction;
  selected: boolean;
  message?: string;
}

export interface InitPreviewSummary {
  migratable: number;
  alreadyRouted: number;
  conflicts: number;
  skipped: number;
}

export interface InitPreview {
  centralPath: string;
  items: InitPreviewItem[];
  summary: InitPreviewSummary;
}

export interface InitRunItem {
  key: string;
  id: string;
  platformId: string;
  action: InitAction;
  status: string;
  message?: string;
}

export interface InitResult {
  backupRoot: string;
  completed: number;
  skipped: number;
  failed: number;
  items: InitRunItem[];
}
