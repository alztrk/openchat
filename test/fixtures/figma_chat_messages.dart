import 'package:openchat/features/chat/domain/chat_message.dart';

const figmaChatMessages = <ChatMessage>[
  ChatMessage(
    id: 'question-about-first-release',
    role: ChatMessageRole.user,
    content: "OpenChat'in ilk sürümünde sohbet ekranında hangi alanlar olmalı?",
  ),
  ChatMessage(
    id: 'first-release-answer',
    role: ChatMessageRole.assistant,
    content:
        'Sohbet akışında geçmişten konuşma açma, mesaj yazma ve yanıt alma '
        'temel akış olsun. Sağlayıcı, model ve akıl yürütme düzeyi giriş '
        'alanında yer alır.\n\n'
        'Dosya değişiklikleri yanıt tamamlandıktan sonra gözden geçirilebilir '
        've gerekirse geri alınabilir.',
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
