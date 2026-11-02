import {
  Braces,
  Check,
  Database,
  FileCode2,
  FileCog,
  FileImage,
  FileText,
  Info,
  LoaderCircle,
  TriangleAlert,
} from 'lucide-react';
import SiC from '@icons-pack/react-simple-icons/icons/SiC';
import SiClojure from '@icons-pack/react-simple-icons/icons/SiClojure';
import SiCplusplus from '@icons-pack/react-simple-icons/icons/SiCplusplus';
import SiCss from '@icons-pack/react-simple-icons/icons/SiCss';
import SiDart from '@icons-pack/react-simple-icons/icons/SiDart';
import SiDotnet from '@icons-pack/react-simple-icons/icons/SiDotnet';
import SiElixir from '@icons-pack/react-simple-icons/icons/SiElixir';
import SiErlang from '@icons-pack/react-simple-icons/icons/SiErlang';
import SiFortran from '@icons-pack/react-simple-icons/icons/SiFortran';
import SiFsharp from '@icons-pack/react-simple-icons/icons/SiFsharp';
import SiGo from '@icons-pack/react-simple-icons/icons/SiGo';
import SiGnubash from '@icons-pack/react-simple-icons/icons/SiGnubash';
import SiHaskell from '@icons-pack/react-simple-icons/icons/SiHaskell';
import SiHtml5 from '@icons-pack/react-simple-icons/icons/SiHtml5';
import SiJavascript from '@icons-pack/react-simple-icons/icons/SiJavascript';
import SiJulia from '@icons-pack/react-simple-icons/icons/SiJulia';
import SiJson from '@icons-pack/react-simple-icons/icons/SiJson';
import SiKotlin from '@icons-pack/react-simple-icons/icons/SiKotlin';
import SiLua from '@icons-pack/react-simple-icons/icons/SiLua';
import SiMarkdown from '@icons-pack/react-simple-icons/icons/SiMarkdown';
import SiNim from '@icons-pack/react-simple-icons/icons/SiNim';
import SiOcaml from '@icons-pack/react-simple-icons/icons/SiOcaml';
import SiOpenjdk from '@icons-pack/react-simple-icons/icons/SiOpenjdk';
import SiPerl from '@icons-pack/react-simple-icons/icons/SiPerl';
import SiPhp from '@icons-pack/react-simple-icons/icons/SiPhp';
import SiPython from '@icons-pack/react-simple-icons/icons/SiPython';
import SiR from '@icons-pack/react-simple-icons/icons/SiR';
import SiRuby from '@icons-pack/react-simple-icons/icons/SiRuby';
import SiRust from '@icons-pack/react-simple-icons/icons/SiRust';
import SiSass from '@icons-pack/react-simple-icons/icons/SiSass';
import SiScala from '@icons-pack/react-simple-icons/icons/SiScala';
import SiSvelte from '@icons-pack/react-simple-icons/icons/SiSvelte';
import SiSwift from '@icons-pack/react-simple-icons/icons/SiSwift';
import SiTypescript from '@icons-pack/react-simple-icons/icons/SiTypescript';
import SiVuedotjs from '@icons-pack/react-simple-icons/icons/SiVuedotjs';
import SiYaml from '@icons-pack/react-simple-icons/icons/SiYaml';
import SiZig from '@icons-pack/react-simple-icons/icons/SiZig';
import { useTranslation } from 'react-i18next';
import { Button, TitleTooltip } from '@shared/components';
import type { ChatFileChange } from '@features/chat/data/chatFileChanges';

type LanguageIconComponent = typeof SiTypescript;

const languageIcons: Record<string, { component: LanguageIconComponent; color: 'default' | 'currentColor' }> = {
  ts: { component: SiTypescript, color: 'default' },
  tsx: { component: SiTypescript, color: 'default' },
  mts: { component: SiTypescript, color: 'default' },
  cts: { component: SiTypescript, color: 'default' },
  js: { component: SiJavascript, color: 'default' },
  jsx: { component: SiJavascript, color: 'default' },
  mjs: { component: SiJavascript, color: 'default' },
  cjs: { component: SiJavascript, color: 'default' },
  py: { component: SiPython, color: 'default' },
  pyi: { component: SiPython, color: 'default' },
  rs: { component: SiRust, color: 'currentColor' },
  go: { component: SiGo, color: 'default' },
  java: { component: SiOpenjdk, color: 'currentColor' },
  c: { component: SiC, color: 'default' },
  h: { component: SiC, color: 'default' },
  cc: { component: SiCplusplus, color: 'default' },
  cpp: { component: SiCplusplus, color: 'default' },
  cxx: { component: SiCplusplus, color: 'default' },
  hpp: { component: SiCplusplus, color: 'default' },
  hxx: { component: SiCplusplus, color: 'default' },
  css: { component: SiCss, color: 'default' },
  scss: { component: SiSass, color: 'default' },
  sass: { component: SiSass, color: 'default' },
  html: { component: SiHtml5, color: 'default' },
  htm: { component: SiHtml5, color: 'default' },
  json: { component: SiJson, color: 'currentColor' },
  jsonc: { component: SiJson, color: 'currentColor' },
  sh: { component: SiGnubash, color: 'default' },
  bash: { component: SiGnubash, color: 'default' },
  zsh: { component: SiGnubash, color: 'default' },
  php: { component: SiPhp, color: 'default' },
  rb: { component: SiRuby, color: 'default' },
  kt: { component: SiKotlin, color: 'default' },
  kts: { component: SiKotlin, color: 'default' },
  swift: { component: SiSwift, color: 'default' },
  dart: { component: SiDart, color: 'default' },
  cs: { component: SiDotnet, color: 'default' },
  csx: { component: SiDotnet, color: 'default' },
  fs: { component: SiFsharp, color: 'default' },
  fsi: { component: SiFsharp, color: 'default' },
  fsx: { component: SiFsharp, color: 'default' },
  vue: { component: SiVuedotjs, color: 'default' },
  svelte: { component: SiSvelte, color: 'default' },
  md: { component: SiMarkdown, color: 'currentColor' },
  mdx: { component: SiMarkdown, color: 'currentColor' },
  yaml: { component: SiYaml, color: 'default' },
  yml: { component: SiYaml, color: 'default' },
  ex: { component: SiElixir, color: 'default' },
  exs: { component: SiElixir, color: 'default' },
  erl: { component: SiErlang, color: 'default' },
  hrl: { component: SiErlang, color: 'default' },
  hs: { component: SiHaskell, color: 'default' },
  lhs: { component: SiHaskell, color: 'default' },
  lua: { component: SiLua, color: 'default' },
  pl: { component: SiPerl, color: 'default' },
  pm: { component: SiPerl, color: 'default' },
  scala: { component: SiScala, color: 'default' },
  zig: { component: SiZig, color: 'default' },
  nim: { component: SiNim, color: 'default' },
  ml: { component: SiOcaml, color: 'default' },
  mli: { component: SiOcaml, color: 'default' },
  f: { component: SiFortran, color: 'default' },
  f90: { component: SiFortran, color: 'default' },
  f95: { component: SiFortran, color: 'default' },
  clj: { component: SiClojure, color: 'default' },
  cljs: { component: SiClojure, color: 'default' },
  cljc: { component: SiClojure, color: 'default' },
  r: { component: SiR, color: 'default' },
  jl: { component: SiJulia, color: 'default' },
};

export function FileTypeIcon({ path, size = 16 }: { path: string; size?: number }) {
  const extension = path.replaceAll('\\', '/').split('/').at(-1)?.split('.').at(-1)?.toLowerCase() ?? '';
  const iconProps = { 'aria-hidden': true as const, className: 'shrink-0 text-secondary-icon', size };
  const languageIcon = languageIcons[extension];
  if (languageIcon) {
    const LanguageIcon = languageIcon.component;
    return (
      <span aria-hidden="true" className="inline-flex shrink-0 items-center justify-center">
        <LanguageIcon className="shrink-0" color={languageIcon.color} size={size} />
      </span>
    );
  }
  if (extension === 'ps1') {
    return <FileCode2 {...iconProps} />;
  }
  if (['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'ico', 'bmp'].includes(extension)) {
    return <FileImage {...iconProps} />;
  }
  if (['toml', 'ini', 'env', 'config', 'lock'].includes(extension)) {
    return <FileCog {...iconProps} />;
  }
  if (extension === 'sql') return <Database {...iconProps} />;
  if (extension === 'xml') return <Braces {...iconProps} />;
  return <FileText {...iconProps} />;
}

export interface FileChangeRowProps {
  change: ChatFileChange;
  isAnyMutating: boolean;
  isSelected: boolean;
  isMutating: boolean;
  onSelect(): void;
  onApprove(): void;
  onRevert(): void;
}

export function FileChangeRow({
  change,
  isAnyMutating,
  isSelected,
  isMutating,
  onSelect,
  onApprove,
  onRevert,
}: FileChangeRowProps) {
  const { t } = useTranslation();
  return (
    <li
      className={`mx-0 flex min-h-12 w-full items-center gap-1.5 border-y border-border px-2 ${isSelected ? 'bg-composer' : ''}`}
    >
      <Button
        aria-pressed={isSelected}
        className="min-h-0 min-w-0 flex-1 justify-start gap-2 bg-transparent py-1 text-left hover:bg-transparent focus-visible:bg-transparent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
        onClick={onSelect}
        size="small"
        title={change.path}
        type="button"
        variant="ghost"
      >
        <FileTypeIcon path={change.path} />
        <span className="min-w-0 flex-1">
          <span className="block truncate text-xs font-medium text-text">{change.path}</span>
        </span>
        {change.addedLines !== null || change.removedLines !== null ? (
          <span className="flex shrink-0 gap-1 text-xs font-semibold">
            {change.addedLines !== null ? <span className="text-success">+{change.addedLines}</span> : null}
            {change.removedLines !== null ? <span className="text-destructive">-{change.removedLines}</span> : null}
          </span>
        ) : null}
      </Button>
      {change.status === 'active' && !change.approved ? (
        <div className="flex shrink-0 items-center gap-1">
          <Button
            aria-label={`${t('fileChangesApprove')}: ${change.path}`}
            className="h-7 px-2 text-xs text-success hover:text-success"
            disabled={isMutating || isAnyMutating}
            onClick={onApprove}
            size="small"
            title={t('fileChangesApprove')}
            variant="ghost"
          >
            <Check aria-hidden="true" size={14} />
            <span>{t('fileChangesApprove')}</span>
          </Button>
          {change.canRevert ? (
            <Button
              aria-label={`${t('fileChangesRevert')}: ${change.path}`}
              className="h-7 px-2 text-xs"
              disabled={isMutating || isAnyMutating}
              onClick={onRevert}
              size="small"
              variant="ghost"
            >
              {isMutating ? (
                <LoaderCircle aria-hidden="true" className="animate-spin motion-reduce:animate-none" size={13} />
              ) : (
                t('fileChangesRevert')
              )}
            </Button>
          ) : null}
        </div>
      ) : change.status === 'conflict' ? (
        <TitleTooltip title={t('fileChangesConflict')}><span aria-label={t('fileChangesConflict')} role="img" >
          <TriangleAlert aria-hidden="true" className="text-destructive" size={16} />
        </span></TitleTooltip>
      ) : !change.diffAvailable ? (
        <TitleTooltip title={t('fileChangesDiffUnavailable')}><span aria-label={t('fileChangesDiffUnavailable')} role="img" >
          <Info aria-hidden="true" className="text-secondary-icon" size={16} />
        </span></TitleTooltip>
      ) : null}
    </li>
  );
}
