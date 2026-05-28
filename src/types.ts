export type PaletteKey = 'cool' | 'warm' | 'slate';
export type Density = 'compact' | 'comfortable';
export type View = 'list' | 'grid';
export type PlatformGroup = 'Core' | 'Coding' | 'Lobster';

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
}

export interface ApiKeyStatus {
  configured: boolean;
}
