import 'dart:async';

import 'package:flutter/material.dart';
import 'package:intl/intl.dart';
import 'package:lucide_icons_flutter/lucide_icons.dart';

import 'package:openchat/app/openchat_page_header.dart';
import 'package:openchat/app/openchat_theme.dart';
import 'package:openchat/app/openchat_toast.dart';
import 'package:openchat/features/chat/data/chat_repository.dart';
import 'package:openchat/features/chat/domain/chat_conversation.dart';
import 'package:openchat/features/chat/domain/chat_workspace.dart';
import 'package:openchat/l10n/openchat_localizations.dart';

class WorkspacesPage extends StatefulWidget {
  const WorkspacesPage({
    required this.repository,
    required this.onCreateWorkspace,
    required this.onRenameWorkspace,
    required this.onDeleteWorkspace,
    required this.onSetConversationWorkspace,
    required this.onCreateConversation,
    required this.onOpenConversation,
    this.pageHeadingFocusNode,
    this.onRetry,
    super.key,
  });

  final ChatRepository? repository;
  final Future<void> Function(String name) onCreateWorkspace;
  final Future<void> Function(String workspaceId, String name)
  onRenameWorkspace;
  final Future<void> Function(String workspaceId) onDeleteWorkspace;
  final Future<void> Function(String conversationId, String? workspaceId)
  onSetConversationWorkspace;
  final ValueChanged<String> onCreateConversation;
  final ValueChanged<String> onOpenConversation;
  final FocusNode? pageHeadingFocusNode;
  final VoidCallback? onRetry;

  @override
  State<WorkspacesPage> createState() => _WorkspacesPageState();
}

class _WorkspacesPageState extends State<WorkspacesPage> {
  int _streamGeneration = 0;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final repository = widget.repository;
    final compact = MediaQuery.sizeOf(context).width < 640;

    return SingleChildScrollView(
      padding: EdgeInsets.fromLTRB(
        compact ? OpenChatSpacing.md : OpenChatSpacing.xl,
        OpenChatSpacing.lg,
        compact ? OpenChatSpacing.md : OpenChatSpacing.xl,
        OpenChatSpacing.section,
      ),
      child: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 960),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              OpenChatPageHeader(
                title: l10n.workspaces,
                description: l10n.workspacesDescription,
                focusNode: widget.pageHeadingFocusNode,
                actions: [
                  FilledButton.icon(
                    onPressed: repository == null
                        ? null
                        : () => unawaited(_editWorkspaceName()),
                    icon: const Icon(LucideIcons.plus),
                    label: Text(l10n.workspaceCreate),
                  ),
                ],
              ),
              const SizedBox(height: OpenChatSpacing.xl),
              if (repository == null)
                _loadFailure(context)
              else
                StreamBuilder<List<ChatWorkspace>>(
                  key: ValueKey<int>(_streamGeneration),
                  stream: repository.watchWorkspaces(),
                  builder: (context, workspaceSnapshot) {
                    if (workspaceSnapshot.hasError) {
                      return _loadFailure(context);
                    }
                    if (!workspaceSnapshot.hasData) {
                      return Center(
                        child: Padding(
                          padding: const EdgeInsets.all(OpenChatSpacing.xl),
                          child: Semantics(
                            liveRegion: true,
                            label: l10n.workspacesLoading,
                            child: const CircularProgressIndicator(),
                          ),
                        ),
                      );
                    }
                    return StreamBuilder<List<ChatConversation>>(
                      stream: repository.watchConversations(),
                      builder: (context, conversationSnapshot) {
                        if (conversationSnapshot.hasError) {
                          return _loadFailure(context);
                        }
                        if (!conversationSnapshot.hasData) {
                          return Center(
                            child: Padding(
                              padding: const EdgeInsets.all(OpenChatSpacing.xl),
                              child: Semantics(
                                liveRegion: true,
                                label: l10n.conversationsLoading,
                                child: const CircularProgressIndicator(),
                              ),
                            ),
                          );
                        }
                        return _workspaceContent(
                          context,
                          workspaceSnapshot.data!,
                          conversationSnapshot.data!,
                        );
                      },
                    );
                  },
                ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _workspaceContent(
    BuildContext context,
    List<ChatWorkspace> workspaces,
    List<ChatConversation> conversations,
  ) {
    final l10n = context.openchatL10n;
    final activeConversations = conversations
        .where((conversation) => !conversation.isArchived)
        .toList(growable: false);
    final unassigned = activeConversations
        .where((conversation) => conversation.productWorkspaceId == null)
        .toList(growable: false);

    if (workspaces.isEmpty && unassigned.isEmpty) {
      return _EmptyWorkspaces(onCreate: () => unawaited(_editWorkspaceName()));
    }

    if (workspaces.isEmpty) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _EmptyWorkspaces(onCreate: () => unawaited(_editWorkspaceName())),
          const SizedBox(height: OpenChatSpacing.lg),
          Text(
            l10n.workspaceUnassignedChats,
            style: Theme.of(context).textTheme.titleMedium,
          ),
          const SizedBox(height: OpenChatSpacing.xs),
          for (final conversation in unassigned)
            _WorkspaceConversationRow(
              conversation: conversation,
              workspaces: workspaces,
              onOpen: () => widget.onOpenConversation(conversation.id),
              onMove: (workspaceId) => unawaited(
                widget.onSetConversationWorkspace(conversation.id, workspaceId),
              ),
              locale: l10n.localeName,
            ),
        ],
      );
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        for (final workspace in workspaces) ...[
          _WorkspaceGroup(
            workspace: workspace,
            conversations: activeConversations
                .where(
                  (conversation) =>
                      conversation.productWorkspaceId == workspace.id,
                )
                .toList(growable: false),
            allWorkspaces: workspaces,
            onCreateConversation: widget.onCreateConversation,
            onOpenConversation: widget.onOpenConversation,
            onSetConversationWorkspace: widget.onSetConversationWorkspace,
            onRename: () => unawaited(_editWorkspaceName(workspace)),
            onDelete: () => unawaited(_confirmDeleteWorkspace(workspace)),
          ),
          const SizedBox(height: OpenChatSpacing.md),
        ],
        if (unassigned.isNotEmpty) ...[
          const SizedBox(height: OpenChatSpacing.md),
          Text(
            l10n.workspaceUnassignedChats,
            style: Theme.of(context).textTheme.titleMedium,
          ),
          const SizedBox(height: OpenChatSpacing.xs),
          for (final conversation in unassigned)
            _WorkspaceConversationRow(
              conversation: conversation,
              workspaces: workspaces,
              onOpen: () => widget.onOpenConversation(conversation.id),
              onMove: (workspaceId) => unawaited(
                widget.onSetConversationWorkspace(conversation.id, workspaceId),
              ),
              locale: l10n.localeName,
            ),
        ],
      ],
    );
  }

  Widget _loadFailure(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    return Container(
      padding: const EdgeInsets.all(OpenChatSpacing.lg),
      decoration: BoxDecoration(
        color: palette.surface,
        borderRadius: BorderRadius.circular(OpenChatRadii.panel),
        border: Border.all(color: palette.border),
      ),
      child: Wrap(
        alignment: WrapAlignment.spaceBetween,
        crossAxisAlignment: WrapCrossAlignment.center,
        spacing: OpenChatSpacing.md,
        runSpacing: OpenChatSpacing.md,
        children: [
          Text(
            l10n.workspaceLoadFailed,
            style: Theme.of(context).textTheme.bodyMedium,
          ),
          OutlinedButton.icon(
            onPressed: widget.onRetry == null
                ? null
                : () {
                    setState(() => _streamGeneration++);
                    widget.onRetry?.call();
                  },
            icon: const Icon(LucideIcons.refreshCw),
            label: Text(l10n.retry),
          ),
        ],
      ),
    );
  }

  Future<void> _editWorkspaceName([ChatWorkspace? workspace]) async {
    final l10n = context.openchatL10n;
    var draftName = workspace?.name ?? '';
    var saving = false;
    String? formError;

    await showDialog<void>(
      context: context,
      builder: (dialogContext) => StatefulBuilder(
        builder: (context, setDialogState) {
          Future<void> submit() async {
            final name = draftName.trim();
            if (name.isEmpty) {
              setDialogState(() => formError = l10n.workspaceNameRequired);
              return;
            }
            setDialogState(() {
              saving = true;
              formError = null;
            });
            try {
              final save = workspace == null
                  ? widget.onCreateWorkspace(name)
                  : widget.onRenameWorkspace(workspace.id, name);
              await save;
              if (dialogContext.mounted) Navigator.pop(dialogContext);
            } on Object catch (error, stackTrace) {
              FlutterError.reportError(
                FlutterErrorDetails(
                  exception: error,
                  stack: stackTrace,
                  library: 'workspaces',
                  context: ErrorDescription('while saving a workspace name'),
                ),
              );
              if (dialogContext.mounted) {
                setDialogState(() {
                  saving = false;
                  formError = l10n.workspaceSaveFailed;
                });
              }
            }
          }

          return AlertDialog(
            title: Text(
              workspace == null ? l10n.workspaceCreate : l10n.workspaceRename,
            ),
            content: TextFormField(
              initialValue: workspace?.name ?? '',
              autofocus: true,
              maxLength: 80,
              textCapitalization: TextCapitalization.sentences,
              decoration: InputDecoration(
                labelText: l10n.workspaceName,
                errorText: formError,
              ),
              onChanged: (value) {
                draftName = value;
                if (formError != null) setDialogState(() => formError = null);
              },
              onFieldSubmitted: (_) => unawaited(submit()),
            ),
            actions: [
              TextButton(
                onPressed: saving ? null : () => Navigator.pop(dialogContext),
                child: Text(l10n.cancel),
              ),
              FilledButton(
                onPressed: saving ? null : () => unawaited(submit()),
                child: saving
                    ? const SizedBox.square(
                        dimension: 16,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : Text(
                        workspace == null
                            ? l10n.workspaceCreateAction
                            : l10n.save,
                      ),
              ),
            ],
          );
        },
      ),
    );
  }

  Future<void> _confirmDeleteWorkspace(ChatWorkspace workspace) async {
    final l10n = context.openchatL10n;
    final shouldDelete = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text(l10n.workspaceDelete),
        content: Text(l10n.workspaceDeleteConfirmation(workspace.name)),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: Text(l10n.cancel),
          ),
          FilledButton.tonal(
            onPressed: () => Navigator.pop(context, true),
            child: Text(l10n.workspaceDelete),
          ),
        ],
      ),
    );
    if (shouldDelete != true || !mounted) return;
    try {
      await widget.onDeleteWorkspace(workspace.id);
    } on Object catch (error, stackTrace) {
      FlutterError.reportError(
        FlutterErrorDetails(
          exception: error,
          stack: stackTrace,
          library: 'workspaces',
          context: ErrorDescription('while deleting a workspace'),
        ),
      );
      if (mounted) {
        showOpenChatToast(
          context,
          l10n.workspaceDeleteFailed,
          type: OpenChatToastType.error,
        );
      }
    }
  }
}

class _WorkspaceGroup extends StatelessWidget {
  const _WorkspaceGroup({
    required this.workspace,
    required this.conversations,
    required this.allWorkspaces,
    required this.onCreateConversation,
    required this.onOpenConversation,
    required this.onSetConversationWorkspace,
    required this.onRename,
    required this.onDelete,
  });

  final ChatWorkspace workspace;
  final List<ChatConversation> conversations;
  final List<ChatWorkspace> allWorkspaces;
  final ValueChanged<String> onCreateConversation;
  final ValueChanged<String> onOpenConversation;
  final Future<void> Function(String conversationId, String? workspaceId)
  onSetConversationWorkspace;
  final VoidCallback onRename;
  final VoidCallback onDelete;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    final locale = l10n.localeName;
    return Material(
      color: palette.surface,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(OpenChatRadii.panel),
        side: BorderSide(color: palette.subtleBorder),
      ),
      clipBehavior: Clip.antiAlias,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(
              OpenChatSpacing.md,
              OpenChatSpacing.xs,
              OpenChatSpacing.xs,
              OpenChatSpacing.xs,
            ),
            child: Wrap(
              alignment: WrapAlignment.spaceBetween,
              crossAxisAlignment: WrapCrossAlignment.center,
              spacing: OpenChatSpacing.sm,
              runSpacing: OpenChatSpacing.xs,
              children: [
                ConstrainedBox(
                  constraints: const BoxConstraints(maxWidth: 520),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        workspace.name,
                        style: Theme.of(context).textTheme.titleMedium,
                      ),
                      const SizedBox(height: OpenChatSpacing.xxs),
                      Text(
                        l10n.workspaceConversationCount(conversations.length),
                        style: Theme.of(context).textTheme.bodySmall,
                      ),
                    ],
                  ),
                ),
                Wrap(
                  spacing: OpenChatSpacing.xs,
                  children: [
                    TextButton.icon(
                      onPressed: () => onCreateConversation(workspace.id),
                      icon: const Icon(LucideIcons.messageSquarePlus),
                      label: Text(l10n.newChat),
                    ),
                    IconButton(
                      tooltip: l10n.workspaceRename,
                      onPressed: onRename,
                      icon: const Icon(LucideIcons.pencil),
                    ),
                    IconButton(
                      tooltip: l10n.workspaceDelete,
                      onPressed: onDelete,
                      icon: Icon(
                        LucideIcons.trash2,
                        color: Theme.of(context).colorScheme.error,
                      ),
                    ),
                  ],
                ),
              ],
            ),
          ),
          Divider(height: 1, color: palette.subtleBorder),
          if (conversations.isEmpty)
            Padding(
              padding: const EdgeInsets.all(OpenChatSpacing.md),
              child: Text(
                l10n.workspaceNoConversations,
                style: Theme.of(context).textTheme.bodyMedium,
              ),
            )
          else
            for (final conversation in conversations)
              _WorkspaceConversationRow(
                conversation: conversation,
                workspaces: allWorkspaces,
                onOpen: () => onOpenConversation(conversation.id),
                onMove: (workspaceId) => unawaited(
                  onSetConversationWorkspace(conversation.id, workspaceId),
                ),
                locale: locale,
              ),
        ],
      ),
    );
  }
}

class _WorkspaceConversationRow extends StatelessWidget {
  const _WorkspaceConversationRow({
    required this.conversation,
    required this.workspaces,
    required this.onOpen,
    required this.onMove,
    this.locale = 'en',
  });

  final ChatConversation conversation;
  final List<ChatWorkspace> workspaces;
  final VoidCallback onOpen;
  final ValueChanged<String?> onMove;
  final String locale;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    return ListTile(
      contentPadding: const EdgeInsets.symmetric(
        horizontal: OpenChatSpacing.md,
      ),
      leading: Icon(
        LucideIcons.messageCircle,
        size: 18,
        color: palette.secondaryIcon,
      ),
      title: Text(
        conversation.title,
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
      ),
      subtitle: Text(
        DateFormat.yMMMd(locale).format(conversation.updatedAt.toLocal()),
      ),
      onTap: onOpen,
      trailing: workspaces.isEmpty
          ? null
          : PopupMenuButton<String>(
              tooltip: l10n.workspaceMoveConversation,
              onSelected: (id) => onMove(id.isEmpty ? null : id),
              itemBuilder: (context) => [
                for (final workspace in workspaces)
                  PopupMenuItem<String>(
                    value: workspace.id,
                    child: Text(workspace.name),
                  ),
                PopupMenuItem<String>(
                  value: '',
                  child: Text(l10n.workspaceNoWorkspace),
                ),
              ],
              icon: const Icon(LucideIcons.folderInput, size: 18),
            ),
    );
  }
}

class _EmptyWorkspaces extends StatelessWidget {
  const _EmptyWorkspaces({required this.onCreate});

  final VoidCallback onCreate;

  @override
  Widget build(BuildContext context) {
    final l10n = context.openchatL10n;
    final palette = OpenChatPalette.of(context);
    return Container(
      alignment: Alignment.center,
      padding: const EdgeInsets.all(OpenChatSpacing.section),
      decoration: BoxDecoration(
        color: palette.surface,
        borderRadius: BorderRadius.circular(OpenChatRadii.panel),
        border: Border.all(color: palette.subtleBorder),
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Icon(LucideIcons.folder, size: 28, color: palette.secondaryIcon),
          const SizedBox(height: OpenChatSpacing.md),
          Text(
            l10n.workspaceEmptyTitle,
            textAlign: TextAlign.center,
            style: Theme.of(context).textTheme.titleLarge,
          ),
          const SizedBox(height: OpenChatSpacing.xs),
          ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 420),
            child: Text(
              l10n.workspaceEmptyDescription,
              textAlign: TextAlign.center,
              style: Theme.of(context).textTheme.bodyMedium,
            ),
          ),
          const SizedBox(height: OpenChatSpacing.lg),
          FilledButton.icon(
            onPressed: onCreate,
            icon: const Icon(LucideIcons.plus),
            label: Text(l10n.workspaceCreateAction),
          ),
        ],
      ),
    );
  }
}
