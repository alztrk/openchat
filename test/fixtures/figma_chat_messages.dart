import 'package:zihora/features/chat/domain/chat_message.dart';

const figmaChatMessages = <ChatMessage>[
  ChatMessage(
    id: 'question-about-first-release',
    role: ChatMessageRole.user,
    content: 'Zihora’nın ilk sürümünde sohbet ekranında hangi alanlar olmalı?',
  ),
  ChatMessage(
    id: 'first-release-answer',
    role: ChatMessageRole.assistant,
    content:
        'İlk sürümde sohbet akışını merkeze alalım: geçmişten bir konuşma açma, '
        'mesaj yazma ve yanıt alma. Sağlayıcı ve model seçimiyle akıl yürütme '
        'düzeyi giriş alanında yer alır.\n\n'
        'Dosya düzenleme bu ilk kapsamın dışında.',
    elapsed: Duration(seconds: 4),
  ),
  ChatMessage(
    id: 'question-about-response-states',
    role: ChatMessageRole.user,
    content:
        'Yanıt hazırlanırken durdurma ve bağlantı hatası da anlaşılır olmalı.',
  ),
  ChatMessage(
    id: 'response-states-answer',
    role: ChatMessageRole.assistant,
    content:
        'Akış sırasında “Yanıt hazırlanıyor” durumu; hata olduğunda ise '
        '“Yeniden dene” eylemi gösterilir.',
    status: ChatMessageStatus.streaming,
  ),
];
