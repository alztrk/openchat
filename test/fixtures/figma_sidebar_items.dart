import 'package:openchat/features/chat/domain/conversation_sidebar_data.dart';

const figmaSidebarProjects = <ConversationSidebarProject>[
  ConversationSidebarProject(
    id: 'openchat-project',
    title: 'OpenChat',
    conversations: [
      ConversationSidebarConversation(
        id: 'first-chat-experience',
        title: 'İlk sohbet deneyimi',
      ),
      ConversationSidebarConversation(
        id: 'model-selection-project',
        title: 'OpenChat’te model seçimi',
      ),
      ConversationSidebarConversation(
        id: 'chat-history-draft',
        title: 'Sohbet geçmişi taslağı',
      ),
    ],
    hasMoreConversations: true,
  ),
];

const figmaPinnedConversations = <ConversationSidebarConversation>[
  ConversationSidebarConversation(
    id: 'chat-interface',
    title: 'Sohbet arayüzü',
  ),
  ConversationSidebarConversation(id: 'model-selection', title: 'Model seçimi'),
];

const figmaConversations = <ConversationSidebarConversation>[
  ConversationSidebarConversation(
    id: 'light-and-dark-theme',
    title: 'Açık ve koyu tema',
  ),
  ConversationSidebarConversation(
    id: 'provider-list',
    title: 'Sağlayıcı listesi',
  ),
  ConversationSidebarConversation(
    id: 'reasoning-level',
    title: 'Akıl yürütme seviyesi',
  ),
  ConversationSidebarConversation(
    id: 'message-sending-flow',
    title: 'Mesaj gönderme akışı',
  ),
  ConversationSidebarConversation(id: 'window-layout', title: 'Pencere düzeni'),
  ConversationSidebarConversation(
    id: 'empty-chat-view',
    title: 'Boş sohbet görünümü',
  ),
  ConversationSidebarConversation(
    id: 'pinned-conversations',
    title: 'Sabitlenen konuşmalar',
  ),
  ConversationSidebarConversation(id: 'response-state', title: 'Yanıt durumu'),
  ConversationSidebarConversation(
    id: 'keyboard-shortcuts',
    title: 'Klavye kısayolları',
  ),
  ConversationSidebarConversation(
    id: 'narrow-window-layout',
    title: 'Dar pencere düzeni',
  ),
  ConversationSidebarConversation(
    id: 'conversation-list-scrolling',
    title: 'Sohbet listesinin kaydırması',
  ),
  ConversationSidebarConversation(
    id: 'composer-empty-state',
    title: 'Composer boş durumu',
  ),
  ConversationSidebarConversation(
    id: 'connection-error-draft',
    title: 'Bağlantı hatası taslağı',
  ),
  ConversationSidebarConversation(
    id: 'image-attachment-state',
    title: 'Görsel ekleme durumu',
  ),
];
