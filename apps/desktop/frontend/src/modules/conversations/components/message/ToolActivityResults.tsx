import { useTranslation } from 'react-i18next';

import type { ToolActivity } from './messageData';
import { GitResult, ReadUrlResult, TerminalResult, WebSearchResult } from './ToolExecutionResults';
import {
  EditResult,
  FileInfoResult,
  FileListingResult,
  ReadResult,
  SearchResult,
  WriteResult,
} from './ToolFileResults';
import { ToolResultNotice } from './ToolSharedComponents';
import { asRecord, getRequestedToolPath, isTerminalTool, translateToolError } from './toolResultUtils';

export { formatToolPath, getRequestedToolPath } from './toolResultUtils';

interface ToolActivityResultsProps {
  activity: ToolActivity;
  onOpenFile?(path: string, content: string): void;
}

export function ToolActivityResults({ activity, onOpenFile }: ToolActivityResultsProps) {
  const { i18n, t } = useTranslation();
  const output = asRecord(activity.output);
  const requestedPath = getRequestedToolPath(activity);
  const errorMessage = translateToolError(output, t);

  if (activity.status === 'denied' || activity.status === 'cancelled') {
    const message =
      activity.status === 'denied' && errorMessage !== null
        ? errorMessage
        : t(activity.status === 'denied' ? 'toolDenied' : 'toolCancelled');
    return <ToolResultNotice message={message} path={requestedPath} />;
  }
  if (activity.name === 'ask_user' && activity.status === 'waitingForUser') {
    return <ToolResultNotice message={t('toolQuestionReplyInPanel')} />;
  }
  if (activity.status === 'failed' || errorMessage) {
    if (isTerminalTool(activity.name) && output !== null) {
      return <TerminalResult activity={activity} output={output} />;
    }
    return <ToolResultNotice isError message={errorMessage ?? t('toolOperationFailed')} path={requestedPath} />;
  }
  if (output === null) {
    const message =
      activity.status === 'running'
        ? 'toolOperationWorking'
        : activity.status === 'awaitingApproval'
          ? 'toolAwaitingApproval'
          : 'toolOperationUnavailable';
    return <ToolResultNotice isWorking={activity.status === 'running'} message={t(message)} />;
  }

  switch (activity.name) {
    case 'list_files':
    case 'glob':
    case 'list_directory':
      return <FileListingResult activity={activity} output={output} requestedPath={requestedPath} />;
    case 'execute_command':
    case 'bash':
    case 'run_project_task':
    case 'send_terminal_input':
      return <TerminalResult activity={activity} output={output} />;
    case 'search_files':
    case 'grep':
      return <SearchResult activity={activity} output={output} />;
    case 'read_file':
    case 'read':
      return <ReadResult onOpenFile={onOpenFile} output={output} />;
    case 'get_file_info':
      return <FileInfoResult output={output} locale={i18n.language} />;
    case 'write_file':
    case 'write':
      return <WriteResult activity={activity} output={output} locale={i18n.language} />;
    case 'edit_file':
    case 'edit':
      return <EditResult activity={activity} output={output} />;
    case 'web_search':
      return <WebSearchResult activity={activity} output={output} />;
    case 'git_status':
    case 'git_diff':
    case 'git_history':
      return <GitResult activity={activity} output={output} locale={i18n.language} />;
    case 'read_url_content':
    case 'read_url':
      return <ReadUrlResult activity={activity} output={output} locale={i18n.language} />;
    default:
      return null;
  }
}
