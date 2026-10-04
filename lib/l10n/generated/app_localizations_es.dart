// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Spanish Castilian (`es`).
class AppLocalizationsEs extends AppLocalizations {
  AppLocalizationsEs([String locale = 'es']) : super(locale);

  @override
  String get appTitle => 'OpenChat';

  @override
  String get newChat => 'Nuevo chat';

  @override
  String get chats => 'Chats';

  @override
  String get collapseSidebars => 'Contraer barras laterales';

  @override
  String get showSidebars => 'Mostrar barras laterales';

  @override
  String get home => 'Inicio';

  @override
  String get extensions => 'Extensiones';

  @override
  String get scheduled => 'Programado';

  @override
  String get design => 'Diseño';

  @override
  String get security => 'Seguridad';

  @override
  String get sectionUnavailable => 'Esta sección aún no está disponible.';

  @override
  String get settings => 'Ajustes';

  @override
  String get settingsDescription => 'Conexiones, apariencia y datos locales';

  @override
  String get connections => 'Conexiones';

  @override
  String get models => 'Modelos';

  @override
  String get modelsDescription =>
      'Gestiona los modelos de proveedores conectados, establece un modelo predeterminado y oculta los que no necesites.';

  @override
  String get localEngines => 'Motores locales';

  @override
  String get localEnginesDescription =>
      'Instala runtimes locales verificados, registra tus propios archivos de modelo y comprueba si un runtime funciona correctamente. Mueve o copia los modelos a una carpeta del motor, o déjalos donde están.';

  @override
  String get localEnginesUnavailable =>
      'El servicio de motores locales aún no está disponible.';

  @override
  String get localEnginesLoadFailed =>
      'No se pudo cargar la información de los motores locales.';

  @override
  String get localEnginesEmpty =>
      'No hay versiones de motores locales disponibles.';

  @override
  String get localEnginesReload => 'Volver a cargar';

  @override
  String localEngineRelease(String tag) {
    return 'Versión $tag';
  }

  @override
  String get localEngineVariants => 'Paquetes';

  @override
  String get localEngineStable => 'Estable';

  @override
  String get localEnginePreview => 'Vista previa';

  @override
  String get localEngineNightly => 'Nightly';

  @override
  String get localEngineRecommended => 'Recomendado';

  @override
  String get localEngineAvailable => 'Disponible';

  @override
  String get localEngineInstalled => 'Instalado';

  @override
  String get localEngineNotInstalled => 'No instalado';

  @override
  String get localEngineBlocked => 'Bloqueado';

  @override
  String get localEngineUnsupportedPlatform => 'Plataforma no compatible';

  @override
  String get localEngineHardwareUnavailable => 'Hardware no disponible';

  @override
  String get localEngineVllmBlockedReason =>
      'El paquete de vLLM depende de PyTorch y de un runtime completo de Python que OpenChat todavía no instala mediante un bloqueo de dependencias completo verificado con hashes. Windows nativo no es compatible oficialmente; la configuración administrada de WSL2 aún no está disponible.';

  @override
  String get localEngineExllamaBlockedReason =>
      'El runtime ExLlamaV3 todavía no está listo para instalarse. OpenChat debe fijar y verificar el conjunto completo de dependencias de TabbyAPI, PyTorch, Triton, Flash Linear Attention y Python antes de ofrecerlo.';

  @override
  String localEngineRuntimeRequirements(String requirements) {
    return 'Requisitos: $requirements';
  }

  @override
  String get localEngineInstall => 'Instalar';

  @override
  String get localEngineInstalling => 'Preparando la instalación...';

  @override
  String get localEngineInstallProgress =>
      'Progreso de instalación del motor local';

  @override
  String get localEngineCancelInstall => 'Cancelar instalación';

  @override
  String get localEngineCancellingInstall => 'Cancelando...';

  @override
  String get localEngineInstallFailed =>
      'No se pudo instalar el motor local. Se actualizó el catálogo.';

  @override
  String get localEngineHealth => 'Estado del runtime';

  @override
  String get localEngineRunning => 'En ejecución';

  @override
  String get localEngineStopped => 'Detenido';

  @override
  String get localEngineUnhealthy => 'No responde';

  @override
  String get localEngineUnavailable => 'Este motor aún no se puede iniciar.';

  @override
  String get localEngineStartModel => 'Iniciar modelo';

  @override
  String get localEngineStopModel => 'Detener motor';

  @override
  String get localModels => 'Modelos registrados';

  @override
  String get localModelsEmpty => 'No hay modelos registrados para este motor.';

  @override
  String get localModelsPageTitle => 'Modelos locales';

  @override
  String get localModelsPageDescription =>
      'Consulta los modelos locales descargados y registrados.';

  @override
  String get localModelsPageEmpty =>
      'Todavía no hay modelos locales registrados.';

  @override
  String get localModelsLoadFailed =>
      'No se pudieron cargar los modelos locales.';

  @override
  String get localModelsRefresh => 'Actualizar';

  @override
  String get localModelsDiscover => 'Buscar modelos';

  @override
  String get localModelAddFile => 'Añadir archivo de modelo';

  @override
  String get localModelAddFolder => 'Añadir carpeta de modelo';

  @override
  String get localModelStorageChoiceTitle => 'Elige dónde guardar el modelo';

  @override
  String localModelStorageChoiceTarget(String folder) {
    return 'Carpeta de modelos seleccionada: $folder';
  }

  @override
  String get localModelDirectoryTitle => 'Carpeta de modelos';

  @override
  String get localModelDirectoryDescription =>
      'Elige dónde se guardan los modelos de este motor. Cambiar la carpeta no mueve los modelos ya registrados.';

  @override
  String get localModelChooseDirectory => 'Elegir carpeta';

  @override
  String get localModelUseDefaultDirectory => 'Usar predeterminada';

  @override
  String get localModelScanDirectory => 'Buscar en la carpeta';

  @override
  String get localModelScanningDirectory => 'Buscando en la carpeta...';

  @override
  String get localModelDiscoveryTitle => 'Se encontraron modelos sin registrar';

  @override
  String localModelDiscoveryPrompt(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other:
          'OpenChat encontró $count modelos compatibles sin registrar en esta carpeta. ¿Quieres registrarlos?',
      one: 'OpenChat encontró 1 modelo compatible sin registrar en esta carpeta. ¿Quieres registrarlo?',
    );
    return '$_temp0';
  }

  @override
  String get localModelDiscoveryTruncated =>
      'La búsqueda alcanzó su límite seguro. Elige una carpeta más pequeña para encontrar más modelos.';

  @override
  String get localModelDiscoveryEmpty =>
      'No se encontraron modelos compatibles nuevos en esta carpeta.';

  @override
  String get localModelDiscoveryRegisterAll => 'Registrar modelos encontrados';

  @override
  String localModelDiscoveryRegistered(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Se registraron $count modelos.',
      one: 'Se registró 1 modelo.',
    );
    return '$_temp0';
  }

  @override
  String localModelDiscoveryPartial(int registered, int total) {
    return 'Se registraron $registered de $total modelos. No se pudieron registrar algunos modelos.';
  }

  @override
  String get localModelDirectoryUnavailable =>
      'Esta carpeta de modelos no está disponible. Elige una carpeta existente a la que OpenChat pueda acceder.';

  @override
  String get localModelDiscoveryFailed =>
      'No se pudo analizar la carpeta de modelos. Comprueba los permisos de acceso e inténtalo de nuevo.';

  @override
  String get localModelMoveToFolder => 'Mover el modelo a esta carpeta';

  @override
  String get localModelCopyToFolder => 'Copiar el modelo a esta carpeta';

  @override
  String get localModelKeepInPlace => 'Dejar el modelo donde está';

  @override
  String get localModelSaving => 'Guardando modelo...';

  @override
  String get localModelTransferError =>
      'No se pudo copiar ni mover el modelo. El modelo original se dejó en su ubicación.';

  @override
  String get localModelTransferRecoveryError =>
      'No se pudo registrar ni restaurar el modelo. Hay una copia completa en la carpeta de modelos de OpenChat; selecciónala allí para registrarla.';

  @override
  String get localModelRemove => 'Quitar registro';

  @override
  String get localModelRemoveConfirmation =>
      'Solo se quitará el registro de OpenChat. El archivo del modelo permanecerá en el disco. ¿Continuar?';

  @override
  String get localModelCancelStart => 'Cancelar inicio';

  @override
  String get localModelStopping => 'Deteniendo el runtime...';

  @override
  String get localModelActionError =>
      'No se pudo completar la acción sobre el modelo local.';

  @override
  String get localModelPathMissing =>
      'No se encontró la ruta del modelo. Devuélvelo a su ubicación o quita su registro.';

  @override
  String get localModelEngineNotReady =>
      'Disponible después de instalar este motor y poder ejecutar modelos.';

  @override
  String get localModelInvalid =>
      'El archivo o la carpeta seleccionados no son un modelo válido para este motor.';

  @override
  String get localModelPathError =>
      'No se pudo acceder al archivo o la carpeta del modelo seleccionados.';

  @override
  String get localModelStoragePathError =>
      'OpenChat no pudo crear sus carpetas de modelos. Comprueba el espacio disponible y los permisos, y vuelve a intentarlo.';

  @override
  String get localModelStorageError =>
      'No se pudo guardar el registro del modelo en la base de datos.';

  @override
  String get localModelStartError =>
      'No se pudo iniciar el modelo. Comprueba la instalación del runtime y el archivo del modelo.';

  @override
  String get localModelStartTimeout =>
      'El modelo no estuvo listo a tiempo. Prueba con un modelo más pequeño o comprueba el hardware.';

  @override
  String get localModelRuntimeUnavailable =>
      'El modelo local dejó de responder. Reinícialo en Ajustes > Motores locales y vuelve a intentarlo.';

  @override
  String get localModelInferenceFailed =>
      'El modelo local no pudo procesar esta solicitud. Comprueba su plantilla de chat y la memoria disponible.';

  @override
  String get localModelSaved => 'Modelo local registrado.';

  @override
  String get localModelRemoved => 'Registro del modelo local eliminado.';

  @override
  String get localEngineStageDownloading => 'Descargando';

  @override
  String get localEngineStageVerifying => 'Verificando';

  @override
  String get localEngineStageExtracting => 'Extrayendo';

  @override
  String get localEngineStagePublishing => 'Finalizando la instalación';

  @override
  String get localEngineStageReady => 'Listo';

  @override
  String get defaultModel => 'Predeterminado';

  @override
  String get setDefaultModel => 'Establecer como predeterminado';

  @override
  String get clearDefaultModel => 'Quitar predeterminado';

  @override
  String defaultModelUpdated(String model) {
    return 'Modelo predeterminado actualizado: $model';
  }

  @override
  String get defaultModelCleared => 'Modelo predeterminado eliminado.';

  @override
  String get hideModel => 'Ocultar';

  @override
  String get showModel => 'Mostrar';

  @override
  String get hiddenModel => 'Oculto';

  @override
  String modelHidden(String model) {
    return 'Modelo oculto: $model';
  }

  @override
  String modelUnhidden(String model) {
    return 'Modelo mostrado: $model';
  }

  @override
  String get noModelsFound => 'No se encontraron modelos.';

  @override
  String get refreshModels => 'Actualizar modelos';

  @override
  String get connectedProvidersModels => 'Modelos de proveedores conectados';

  @override
  String get noConnectedProviders =>
      'Todavía no hay proveedores conectados. Conecta cuentas o añade claves de API en la pestaña Conexiones.';

  @override
  String get sharedInstructions => 'Instrucciones compartidas';

  @override
  String get sharedInstructionsDescription =>
      'Estas instrucciones se envían a todos los proveedores conectados. El acceso a las herramientas de archivos sigue el ajuste Acceso a herramientas.';

  @override
  String get sharedInstructionsHint =>
      'Describe cómo quieres que se escriban las respuestas...';

  @override
  String get sharedInstructionsLoadFailed =>
      'No se pudieron cargar las instrucciones compartidas. Vuelve a intentarlo.';

  @override
  String get sharedInstructionsSaveFailed =>
      'No se pudieron guardar las instrucciones compartidas.';

  @override
  String get sharedInstructionsSaved => 'Instrucciones compartidas guardadas.';

  @override
  String get sharedInstructionsTooLong =>
      'Las instrucciones compartidas no pueden superar los 4096 caracteres.';

  @override
  String get chatGptProvider => 'ChatGPT';

  @override
  String get openCodeProvider => 'OpenCode';

  @override
  String get geminiProvider => 'Gemini';

  @override
  String get groqProvider => 'Groq';

  @override
  String get cerebrasProvider => 'Cerebras';

  @override
  String get openRouterProvider => 'OpenRouter';

  @override
  String get mistralProvider => 'Mistral';

  @override
  String get geminiApiDescription =>
      'Añade una clave de API de Google AI Studio para usar los modelos disponibles en tu cuenta. El acceso gratuito depende del modelo y de tu cuota.';

  @override
  String get groqApiDescription =>
      'Usa los modelos habilitados para tu cuenta con una clave de API de Groq. Los precios y límites de uso varían según el plan y el modelo.';

  @override
  String get cerebrasApiDescription =>
      'Usa los modelos compatibles con herramientas disponibles para tu cuenta mediante una clave de API de Cerebras. El acceso, los precios y los límites varían según el modelo y la cuenta.';

  @override
  String get openRouterApiDescription =>
      'Aquí solo aparecen los modelos con coste de 0 \$ para entrada y salida que admiten texto y herramientas. El acceso y los límites reales pueden cambiar.';

  @override
  String get mistralApiDescription =>
      'Añade una clave de API de Mistral para usar los modelos de chat disponibles en tu cuenta. El acceso gratuito, los precios y los límites dependen de tu plan de Mistral.';

  @override
  String get geminiUnpaidDataNotice =>
      'En el nivel gratuito de la API de Gemini, Google puede usar el contenido enviado para mejorar sus productos. Revisa las condiciones de uso de datos de Google antes de enviar información sensible.';

  @override
  String get providerApiKey => 'Clave de API';

  @override
  String get providerKeySaved =>
      'La clave de API se guarda de forma segura en este dispositivo.';

  @override
  String providerKeySavedSuffix(Object suffix) {
    return 'La clave de API terminada en ••••$suffix se guarda de forma segura en este dispositivo.';
  }

  @override
  String get providerNoKey => 'No hay ninguna clave de API conectada.';

  @override
  String get providerKeyInvalid =>
      'Introduce una clave de API válida para este proveedor. No incluyas espacios; la clave puede tener hasta 4096 caracteres.';

  @override
  String get providerKeyStorageFailed =>
      'No se pudo leer ni guardar la clave de API de forma segura.';

  @override
  String get favoriteModels => 'Favoritos';

  @override
  String get favoriteModelsEmpty => 'Todavía no hay modelos favoritos.';

  @override
  String get modelSearchHint => 'Buscar modelos...';

  @override
  String get modelSearchNoResults => 'Ningún modelo coincide con tu búsqueda.';

  @override
  String get addModelFavorite => 'Añadir a modelos favoritos';

  @override
  String get removeModelFavorite => 'Quitar de favoritos';

  @override
  String get openCodeConsole => 'Consola de OpenCode';

  @override
  String get openCodeConsoleDescription =>
      'Los modelos gratuitos funcionan sin clave. Añade una clave de API de la Consola para los modelos de pago; cada solicitud se cobra al saldo de tu Consola.';

  @override
  String openCodeKeySaved(Object suffix) {
    return 'La clave de API de la Consola terminada en ••••$suffix se guarda de forma segura en este dispositivo.';
  }

  @override
  String get openCodeNoKey =>
      'No hay clave de API de la Consola. Los modelos gratuitos están disponibles.';

  @override
  String get openCodeApiKey => 'Clave de API de la Consola de OpenCode';

  @override
  String get openCodeKeyInvalid =>
      'Introduce una clave de API no vacía, sin espacios ni saltos de línea (hasta 4096 caracteres).';

  @override
  String get openCodeKeyStorageFailed =>
      'No se pudo guardar de forma segura la clave de API de la Consola.';

  @override
  String get openCodePaidModel => 'De pago';

  @override
  String get openCodeFreeModel => 'Gratuito';

  @override
  String get modelSourceApi => 'API';

  @override
  String get modelSourceOAuth => 'OAuth';

  @override
  String get openCodeFreeModels => 'Modelos gratuitos';

  @override
  String get openCodeApiModels => 'Modelos de API';

  @override
  String modelContextWindow(String value) {
    return 'Ventana de contexto · $value tokens';
  }

  @override
  String openCodeModelContextWindow(String value) {
    return 'Catálogo de OpenCode (Models.dev) · contexto: $value tokens';
  }

  @override
  String get add => 'Añadir';

  @override
  String get edit => 'Editar';

  @override
  String get exportConversation => 'Exportar conversación';

  @override
  String get deleteConversation => 'Eliminar conversación';

  @override
  String get confirmDeleteConversationTitle => '¿Eliminar esta conversación?';

  @override
  String confirmDeleteConversation(String title) {
    return 'Esto eliminará permanentemente “$title” y todos sus mensajes de este dispositivo.';
  }

  @override
  String get stopResponseBeforeDelete =>
      'Detén la respuesta activa antes de eliminar esta conversación.';

  @override
  String get conversationDeleted => 'Conversación eliminada.';

  @override
  String get conversationDeleteFailed =>
      'No se pudo eliminar la conversación. Vuelve a intentarlo.';

  @override
  String get conversationExported =>
      'Conversación exportada como archivo Markdown.';

  @override
  String get conversationExportFailed =>
      'No se pudo exportar la conversación. Vuelve a intentarlo.';

  @override
  String get conversationExportProvider => 'Proveedor';

  @override
  String get conversationExportModel => 'Modelo';

  @override
  String get conversationExportCreated => 'Creada';

  @override
  String get conversationExportStatus => 'Estado';

  @override
  String get toolPermissions => 'Acceso a herramientas';

  @override
  String get toolPermissionsDescription =>
      'Elige dónde pueden operar las herramientas de archivos de la IA y si cada llamada requiere tu aprobación.';

  @override
  String get toolPermissionRequireApproval => 'Pedir aprobación';

  @override
  String get toolPermissionRequireApprovalDescription =>
      'Las herramientas de archivos preguntan antes de cada llamada y están limitadas a la carpeta del proyecto y %LOCALAPPDATA%\\OpenChat. La ejecución de comandos no está disponible.';

  @override
  String get toolPermissionFullAccess => 'Acceso completo';

  @override
  String get toolPermissionFullAccessDescription =>
      'Las herramientas de archivos pueden leer y cambiar archivos en cualquier carpeta sin preguntar. La ejecución de comandos no está disponible.';

  @override
  String get toolPermissionSettingsLoadFailed =>
      'No se pudieron cargar los ajustes de acceso a herramientas.';

  @override
  String get toolPermissionSettingsSaveFailed =>
      'No se pudieron guardar los ajustes de acceso a herramientas. Vuelve a intentarlo.';

  @override
  String get toolPermissionRequestTitle => 'Permiso de herramienta';

  @override
  String get toolPermissionRequestDescription =>
      'La IA quiere usar esta herramienta en la ubicación seleccionada. El permiso solo se aplica a esta llamada.';

  @override
  String get toolPermissionRequestExpired =>
      'Esta solicitud de permiso de herramienta ya no está activa.';

  @override
  String get toolPermissionResponseFailed =>
      'No se pudo enviar tu elección. Puedes volver a intentarlo.';

  @override
  String get toolPermissionContent => 'Contenido que se escribirá';

  @override
  String get toolPermissionOldText => 'Texto que se buscará';

  @override
  String get toolPermissionNewText => 'Texto de reemplazo';

  @override
  String get toolPermissionQuery => 'Texto de búsqueda';

  @override
  String get toolPermissionOffset => 'Posición inicial';

  @override
  String get toolPermissionLimit => 'Máximo de resultados';

  @override
  String get toolPermissionStartLine => 'Línea inicial';

  @override
  String get toolPermissionLineCount => 'Número de líneas';

  @override
  String get toolPermissionIncludeHidden => 'Incluir archivos ocultos';

  @override
  String get commonYes => 'Sí';

  @override
  String get commonNo => 'No';

  @override
  String get toolPermissionTarget => 'Ubicación a la que se accederá';

  @override
  String get toolPermissionTool => 'Herramienta';

  @override
  String get toolPermissionArguments => 'Detalles de la solicitud';

  @override
  String get toolPermissionDeny => 'Denegar';

  @override
  String get toolPermissionStopResponse => 'Detener respuesta';

  @override
  String get toolPermissionAllowOnce => 'Permitir esta llamada';

  @override
  String get toolDenied => 'Denegado';

  @override
  String get toolCancelled => 'Cancelado';

  @override
  String get toolAwaitingApproval => 'Esperando aprobación';

  @override
  String get conversationExportToolActivity => 'Actividad de herramientas';

  @override
  String get responseReplaceFailed =>
      'La nueva respuesta se guardó, pero no se pudo reemplazar la respuesta anterior.';

  @override
  String get responseRetryNotCompleted =>
      'La nueva respuesta no terminó. Se conservó la respuesta anterior.';

  @override
  String get responseRetryCleanupFailed =>
      'No se pudo limpiar el reintento. Actualiza el historial de la conversación.';

  @override
  String get responseInProgress => 'Respuesta en curso';

  @override
  String get responseRetryUnavailable =>
      'Esta respuesta no se puede reintentar. Inicia un mensaje nuevo.';

  @override
  String get providerRateLimited =>
      'El proveedor ha informado de un límite de uso. Vuelve a intentarlo más tarde.';

  @override
  String get providerAuthenticationRequired =>
      'El proveedor rechazó la solicitud. Comprueba la conexión y el acceso al modelo.';

  @override
  String get providerRequestFailed =>
      'El proveedor no pudo completar la respuesta. Tus mensajes guardados siguen disponibles.';

  @override
  String get contextWindowExceeded =>
      'La conversación es demasiado grande para la ventana de contexto de este modelo. Acorta el último mensaje o elige un modelo con una ventana de contexto mayor. Tu historial de chat está guardado.';

  @override
  String get openCodeFreeTierRestricted =>
      'Los modelos gratuitos de OpenCode solo están disponibles dentro de la aplicación OpenCode.';

  @override
  String get apiKey => 'Clave de API';

  @override
  String get apiKeyInputLabel => 'Clave de API';

  @override
  String get apiKeyRequired => 'Introduce una clave de API.';

  @override
  String get apiKeyInvalidFormat =>
      'Introduce una clave de API de OpenAI válida. Debe comenzar por sk-.';

  @override
  String get apiKeyAlreadySaved => 'Esta clave de API ya está guardada.';

  @override
  String get apiKeySaved =>
      'Clave de API guardada de forma segura en este dispositivo.';

  @override
  String get apiKeySaveFailed =>
      'No se pudo guardar la clave de API de forma segura. Vuelve a intentarlo.';

  @override
  String get apiKeyLoadFailed =>
      'No se pudieron cargar las claves de API guardadas.';

  @override
  String get savedApiKey => 'Clave guardada';

  @override
  String savedApiKeyWithSuffix(String suffix) {
    return 'Clave terminada en ••••$suffix';
  }

  @override
  String get showApiKey => 'Mostrar clave de API';

  @override
  String get hideApiKey => 'Ocultar clave de API';

  @override
  String get retry => 'Volver a intentar';

  @override
  String get save => 'Guardar';

  @override
  String get saving => 'Guardando...';

  @override
  String get oauth => 'OAuth';

  @override
  String get oauthSigningIn => 'Iniciando sesión...';

  @override
  String get oauthBrowserWaiting =>
      'Completa el inicio de sesión en el navegador.';

  @override
  String get oauthConnectionsLoadFailed =>
      'No se pudieron cargar las conexiones OAuth de ChatGPT.';

  @override
  String get oauthSignInFailed =>
      'No se pudo completar el inicio de sesión de ChatGPT. Comprueba el navegador y vuelve a intentarlo.';

  @override
  String get oauthConnectionAdded => 'Cuenta de ChatGPT conectada.';

  @override
  String get oldCredentialCleanupFailed =>
      'La cuenta se conectó, pero no se pudo eliminar una credencial guardada anterior. Reinicia OpenChat y vuelve a intentarlo.';

  @override
  String get connectionSelectionFailed =>
      'No se pudo seleccionar la cuenta de ChatGPT.';

  @override
  String get removeChatGptConnection => 'Quitar conexión de ChatGPT';

  @override
  String get removeConnectionAction => 'Quitar';

  @override
  String confirmRemoveConnection(String name) {
    return '¿Quitar $name y sus credenciales guardadas? Los chats y mensajes existentes permanecerán en este dispositivo.';
  }

  @override
  String get connectionRemoveSucceeded =>
      'Conexión eliminada. Los chats y mensajes existentes siguen disponibles.';

  @override
  String get connectionRemoveFailed =>
      'No se pudo eliminar la conexión. Vuelve a intentarlo.';

  @override
  String get workspaceSelectionFailed =>
      'No se pudo seleccionar el espacio de trabajo de ChatGPT.';

  @override
  String get chatGptAccount => 'Cuenta de ChatGPT';

  @override
  String get planUnavailable => 'Plan no disponible';

  @override
  String accountPlan(String plan) {
    return 'Plan: $plan';
  }

  @override
  String get connectionNeedsSignIn =>
      'Vuelve a iniciar sesión para usar esta cuenta.';

  @override
  String get connectionSelected => 'Seleccionada';

  @override
  String get useConnection => 'Usar cuenta';

  @override
  String get selectWorkspace => 'Elegir un espacio de trabajo';

  @override
  String get workspaceWithoutName => 'Espacio de trabajo';

  @override
  String get workspace => 'Espacio de trabajo';

  @override
  String get workspaceUnavailable =>
      'No hay información del espacio de trabajo disponible.';

  @override
  String get selectAccountForWorkspace =>
      'Selecciona esta cuenta para elegir su espacio de trabajo.';

  @override
  String get accountEmailUnavailable => 'Correo electrónico no disponible';

  @override
  String accountUsage(String plan) {
    return 'Uso · $plan';
  }

  @override
  String get refreshUsage => 'Actualizar uso';

  @override
  String get ordinaryUsageAvailable => 'El uso normal está disponible.';

  @override
  String get ordinaryUsageUnavailable =>
      'El uso normal no está disponible actualmente.';

  @override
  String get ordinaryUsageUnknown =>
      'No se pudo determinar la disponibilidad de uso.';

  @override
  String usageUpdatedAt(String time) {
    return 'Actualizado $time';
  }

  @override
  String get usageLoadFailed => 'No se pudo cargar la información de uso.';

  @override
  String get usageFiveHour => '5 horas';

  @override
  String get usageWeekly => 'Semanal';

  @override
  String get usageMonthly => 'Mensual';

  @override
  String workspaceNumbered(int number) {
    return 'Espacio de trabajo $number';
  }

  @override
  String quotaResetsAt(String time) {
    return 'Se restablece $time';
  }

  @override
  String creditExpiresAt(String time) {
    return 'Caduca $time';
  }

  @override
  String creditGrantedAt(String time) {
    return 'Concedido $time';
  }

  @override
  String get noResetCredits =>
      'No hay créditos de restablecimiento disponibles.';

  @override
  String usageUsedPercent(String percent) {
    return '$percent% usado';
  }

  @override
  String get resetCreditCountUnavailable =>
      'No se puede consultar el número de créditos de restablecimiento.';

  @override
  String resetCreditsAvailable(int count) {
    return 'Créditos de restablecimiento disponibles: $count';
  }

  @override
  String get resetCredit => 'Crédito de restablecimiento';

  @override
  String get statusUnavailable => 'Estado no disponible';

  @override
  String get resetCreditDetailsUnavailable =>
      'No se devolvieron los detalles del crédito de restablecimiento.';

  @override
  String get resetCreditAvailableStatus => 'Disponible';

  @override
  String get useResetCredit => 'Usar crédito';

  @override
  String get resetCreditRedeeming => 'Usando…';

  @override
  String get confirmResetCreditTitle =>
      '¿Usar este crédito de restablecimiento?';

  @override
  String get confirmResetCreditMessage =>
      'Se enviará una solicitud para usar un crédito de restablecimiento. Esta acción no se puede deshacer. ¿Continuar?';

  @override
  String get confirmResetCreditAction => 'Usar crédito';

  @override
  String get resetCreditApplied => 'Se restableció el límite de uso.';

  @override
  String get resetCreditAlreadyUsed =>
      'Este crédito de restablecimiento ya se ha usado.';

  @override
  String get resetCreditNothingToReset =>
      'Ahora mismo no hay ningún límite de uso que restablecer.';

  @override
  String get resetCreditNoLongerAvailable =>
      'Este crédito de restablecimiento ya no está disponible. Actualiza la información de uso.';

  @override
  String get resetCreditOutcomeUnknown =>
      'No se pudo confirmar el resultado. Actualiza la información de uso antes de volver a usar este crédito.';

  @override
  String get resetCreditRefreshRequired =>
      'No se pudo confirmar el resultado anterior. Actualiza la información de uso antes de volver a intentarlo.';

  @override
  String get resetCreditRejected =>
      'ChatGPT no aceptó la solicitud de restablecimiento para esta cuenta.';

  @override
  String get resetCreditSignInRequired =>
      'Vuelve a iniciar sesión en tu cuenta de ChatGPT y vuelve a intentarlo.';

  @override
  String get noChatGptConnections => 'Todavía no hay conexiones de ChatGPT.';

  @override
  String get apiKeyConnectionUnavailable =>
      'Todavía no se ha añadido una conexión con clave de API.';

  @override
  String get oauthConnectionUnavailable =>
      'Todavía no se ha añadido una conexión OAuth.';

  @override
  String get titleGenerationTarget => 'Títulos automáticos de chats';

  @override
  String get titleGenerationTargetDescription =>
      'Cuando la cuenta de la conversación no tiene otro modelo disponible, OpenChat puede usar aquí la cuenta que elijas. Omitirá la generación del título si el uso normal no está disponible.';

  @override
  String get titleUseConversationAccount => 'Usar la cuenta de la conversación';

  @override
  String get titleAccountUnavailable =>
      'La cuenta seleccionada para los títulos no está disponible';

  @override
  String get titleWorkspaceHint =>
      'Elige un espacio de trabajo para los títulos';

  @override
  String get titleWorkspaceRequired =>
      'Elige un espacio de trabajo antes de que esta cuenta pueda generar títulos.';

  @override
  String get titlePreferenceLoadFailed =>
      'No se pudo cargar la preferencia de cuenta para títulos.';

  @override
  String get titlePreferenceSaveFailed =>
      'No se pudo guardar la preferencia de cuenta para títulos.';

  @override
  String get appearance => 'Apariencia';

  @override
  String get themeSettingDescription => 'Elige el aspecto de la aplicación.';

  @override
  String get conversationWidth => 'Ancho de respuesta';

  @override
  String get conversationWidthDescription =>
      'Elige el ancho de línea usado para las respuestas del chat.';

  @override
  String get widthNarrow => 'Estrecho';

  @override
  String get widthNormal => 'Normal';

  @override
  String get widthWide => 'Ancho';

  @override
  String get conversationTextSize => 'Tamaño del texto';

  @override
  String get conversationTextSizeDescription =>
      'Elige el tamaño de texto usado en toda la aplicación.';

  @override
  String get textSizeSmall => 'Pequeño';

  @override
  String get textSizeNormal => 'Normal';

  @override
  String get textSizeLarge => 'Grande';

  @override
  String get appFont => 'Fuente de la aplicación';

  @override
  String get appFontDescription => 'Elige el tipo de letra usado en OpenChat.';

  @override
  String get appearancePreferenceSaveFailed =>
      'No se pudo guardar la preferencia de apariencia. Vuelve a intentarlo.';

  @override
  String get language => 'Idioma de la aplicación';

  @override
  String get languageSettingDescription =>
      'Elige el idioma usado por la aplicación.';

  @override
  String get systemLanguage => 'Idioma del dispositivo';

  @override
  String get englishLanguage => 'Inglés';

  @override
  String get turkishLanguage => 'Turco';

  @override
  String get spanishLanguage => 'Español';

  @override
  String get germanLanguage => 'Alemán';

  @override
  String get frenchLanguage => 'Francés';

  @override
  String get languageSaveFailed =>
      'No se pudo guardar la preferencia de idioma. Vuelve a intentarlo.';

  @override
  String get systemTheme => 'Sistema';

  @override
  String get lightTheme => 'Claro';

  @override
  String get darkTheme => 'Oscuro';

  @override
  String get localData => 'Datos locales';

  @override
  String get conversationHistory => 'Historial de chats';

  @override
  String get historyDeviceDescription =>
      'Las conversaciones se almacenan en este dispositivo.';

  @override
  String get historyDeviceStatus => 'En este dispositivo';

  @override
  String get historyCheckingDescription =>
      'Preparando el historial de chats local.';

  @override
  String get historyCheckingStatus => 'Preparando';

  @override
  String get historyStorageUnavailableDescription =>
      'No se pudo abrir el historial de chats local.';

  @override
  String get historyStorageCorruptDescription =>
      'La base de datos local está dañada. No se aplicaron cambios de esquema. Restaura una copia verificada para continuar.';

  @override
  String get historyStorageBackupFailedDescription =>
      'OpenChat no pudo verificar una copia de seguridad antes de la actualización y la detuvo. Comprueba el espacio disponible y vuelve a intentarlo.';

  @override
  String get historyStorageUnavailableStatus => 'No disponible';

  @override
  String get historyLoading => 'Cargando conversaciones...';

  @override
  String get historyLoadFailed =>
      'No se pudo cargar el historial de chats. Reinicia la aplicación.';

  @override
  String get messageHistoryLoadFailed =>
      'No se pudieron cargar los mensajes de esta conversación.';

  @override
  String get messageHistoryLoading =>
      'Cargando los mensajes de la conversación.';

  @override
  String get clearConversationHistory => 'Borrar todo el historial';

  @override
  String get clearConversationHistoryDescription =>
      'Elimina permanentemente los chats y mensajes almacenados en este dispositivo.';

  @override
  String get confirmClearHistoryTitle => '¿Borrar todo el historial de chats?';

  @override
  String get confirmClearHistoryBody =>
      'Esto eliminará permanentemente todos los chats y mensajes almacenados en este dispositivo. Esta acción no se puede deshacer.';

  @override
  String get cancel => 'Cancelar';

  @override
  String get deleteAll => 'Eliminar todo';

  @override
  String get clearingHistory => 'Eliminando...';

  @override
  String get clearHistorySucceeded => 'Se eliminó el historial de chats.';

  @override
  String get clearHistoryFailed =>
      'No se pudo eliminar el historial de chats. Vuelve a intentarlo.';

  @override
  String get themeSaveFailed =>
      'No se pudo guardar la preferencia de tema. Vuelve a intentarlo.';

  @override
  String get searchChats => 'Buscar chats';

  @override
  String get searchChatsHint => 'Buscar en tus chats';

  @override
  String get projects => 'Proyectos';

  @override
  String get noProjects => 'Todavía no hay proyectos';

  @override
  String get createProject => 'Crear proyecto';

  @override
  String get projectName => 'Nombre del proyecto';

  @override
  String get projectNameRequired => 'Introduce un nombre para el proyecto.';

  @override
  String get projectFolder => 'Carpeta del proyecto';

  @override
  String get chooseProjectFolder => 'Elegir carpeta';

  @override
  String get projectFolderNotSelected => 'Elige una carpeta para continuar.';

  @override
  String get projectFolderSelectionFailed =>
      'No se pudo seleccionar la carpeta.';

  @override
  String get projectCreateFailed => 'No se pudo crear el proyecto.';

  @override
  String get projectCreated => 'Proyecto creado.';

  @override
  String get projectLoadFailed => 'No se pudieron cargar los proyectos.';

  @override
  String get projectMoveFailed => 'No se pudo mover el chat al proyecto.';

  @override
  String get pinnedChats => 'Fijados';

  @override
  String get noPinnedChats => 'Todavía no hay chats fijados';

  @override
  String get noChatsTitle => 'Todavía no hay chats';

  @override
  String get noChatsSearchTitle => 'No hay chats que buscar';

  @override
  String get showMore => 'Mostrar más';

  @override
  String get projectOptions => 'Opciones del proyecto';

  @override
  String get newProjectConversation => 'Iniciar un chat de proyecto nuevo';

  @override
  String get newConversation => 'Iniciar una conversación nueva';

  @override
  String get conversationTitle => 'Nuevo chat';

  @override
  String get conversationMemory => 'Memoria de la conversación';

  @override
  String get conversationMemoryDescription =>
      'Revisa el contexto compacto de esta conversación y busca en sus mensajes anteriores.';

  @override
  String conversationMemoryCurrentConversation(String title) {
    return 'Conversación seleccionada: $title';
  }

  @override
  String get conversationMemoryNoConversation =>
      'Abre una conversación para consultar su memoria.';

  @override
  String get contextUsageTitle => 'Uso del contexto';

  @override
  String contextUsageUsed(String count) {
    return 'Uso: aproximadamente $count tokens';
  }

  @override
  String contextUsageSummary(String used, String limit, String percent) {
    return '~$used / $limit tokens ($percent)';
  }

  @override
  String contextUsageModelLimit(String count) {
    return '$count tokens';
  }

  @override
  String get contextUsageNoModelLimit => 'Límite del modelo desconocido.';

  @override
  String contextUsageProviderMeasurement(String count) {
    return 'Última medición del proveedor: $count tokens';
  }

  @override
  String contextUsageInstructionsEstimate(String count, String percent) {
    return 'Instrucciones: $count tokens · $percent';
  }

  @override
  String contextUsageToolDefinitionsEstimate(String count, String percent) {
    return 'Definiciones de herramientas: $count tokens · $percent';
  }

  @override
  String contextUsageMessagesEstimate(String count, String percent) {
    return 'Mensajes: $count tokens · $percent';
  }

  @override
  String contextUsageAttachmentsEstimate(String count, String percent) {
    return 'Archivos adjuntos: $count tokens · $percent';
  }

  @override
  String contextUsageAttachmentEstimate(
    String name,
    String count,
    String percent,
  ) {
    return '$name: $count tokens · $percent';
  }

  @override
  String contextUsageDraftAttachment(String name) {
    return 'Borrador · $name';
  }

  @override
  String contextUsageUserMessagesEstimate(String count, String percent) {
    return 'Usuario: $count tokens · $percent';
  }

  @override
  String contextUsageAssistantMessagesEstimate(String count, String percent) {
    return 'Asistente: $count tokens · $percent';
  }

  @override
  String contextUsageToolsEstimate(String count, String percent) {
    return 'Uso de herramientas: $count tokens · $percent';
  }

  @override
  String contextUsageToolUsageEstimate(
    String name,
    String count,
    String percent,
  ) {
    return '$name: $count tokens · $percent';
  }

  @override
  String contextUsageMemoryEstimate(String count, String percent) {
    return 'Memoria compactada: $count tokens · $percent';
  }

  @override
  String contextUsageDraftEstimate(String count, String percent) {
    return 'Borrador: $count tokens · $percent';
  }

  @override
  String contextUsageFreeSpaceEstimate(String count, String percent) {
    return 'Espacio libre: $count tokens · $percent';
  }

  @override
  String get contextUsageOverLimit => 'Se ha superado el límite del modelo.';

  @override
  String get contextUsageMeasurementUnavailable =>
      'No se pudieron cargar los detalles de memoria.';

  @override
  String get contextUsageInstructionUnavailable =>
      'No se pudieron cargar los detalles de las instrucciones.';

  @override
  String get contextUsageConfigurationUnavailable =>
      'No se pudieron cargar las estimaciones de instrucciones y definiciones de herramientas.';

  @override
  String get contextUsageConfigurationLoading =>
      'Preparando las estimaciones de instrucciones y definiciones de herramientas…';

  @override
  String get conversationMemorySemanticTitle => 'Búsqueda semántica';

  @override
  String get conversationMemorySemanticDescription =>
      'Descarga un modelo multilingüe de unos 136 MB para encontrar mensajes anteriores expresados de otra forma.';

  @override
  String get conversationMemorySemanticPrepare => 'Preparar';

  @override
  String get conversationMemorySemanticChecking =>
      'Comprobando la búsqueda semántica local…';

  @override
  String get conversationMemorySemanticPreparing =>
      'Descargando y verificando el modelo…';

  @override
  String conversationMemorySemanticDownloadProgress(
    String percent,
    String downloaded,
    String total,
  ) {
    return '$percent% descargado · $downloaded / $total MB';
  }

  @override
  String get conversationMemorySemanticIndexing =>
      'Creando el índice del archivo local…';

  @override
  String get conversationMemorySemanticCancelling => 'Cancelando la descarga…';

  @override
  String get conversationMemorySemanticDownloadCancelled =>
      'Descarga cancelada. La búsqueda por palabras clave sigue disponible.';

  @override
  String get conversationMemorySemanticPrepareFailed =>
      'No se pudo preparar el modelo. Vuelve a intentarlo; se reutilizarán las partes descargadas válidas.';

  @override
  String get conversationMemorySemanticKeywordSearchFallback =>
      'La búsqueda por palabras clave sigue disponible antes de preparar el modelo.';

  @override
  String get conversationMemorySemanticReady =>
      'La búsqueda semántica local está lista';

  @override
  String get conversationMemorySemanticIndexNotice =>
      'El modelo se ejecuta en este dispositivo. La primera búsqueda puede indexar localmente mensajes antiguos y detalles de herramientas guardados, y tardar un poco más.';

  @override
  String get conversationMemorySummaryTitle => 'Contexto compactado';

  @override
  String get conversationMemoryNoSummary =>
      'Todavía no hay ningún resumen compactado.';

  @override
  String get conversationMemoryCheckpointDescription =>
      'El proveedor guarda el contexto como un punto de control reutilizable en lugar de texto de resumen legible. El historial completo de mensajes permanece en el archivo.';

  @override
  String conversationMemoryLastPromptTokens(
    String provider,
    String model,
    String count,
  ) {
    return 'Última solicitud · $provider · $model · $count tokens de entrada';
  }

  @override
  String get conversationMemorySearchTitle => 'Buscar en el archivo';

  @override
  String get conversationMemorySearchHint =>
      'Introduce un tema o una frase anterior...';

  @override
  String get conversationMemorySearchAction => 'Buscar';

  @override
  String get conversationMemorySearchQueryTooShort =>
      'Introduce al menos dos caracteres para buscar.';

  @override
  String get conversationMemorySearchInstruction =>
      'Solo se buscan mensajes completados y resultados de herramientas dentro de esta conversación.';

  @override
  String get conversationMemorySearchNoResults =>
      'No hay entradas coincidentes en el archivo.';

  @override
  String get conversationMemorySearchFailed =>
      'No se pudo buscar en el archivo de la conversación. Vuelve a intentarlo.';

  @override
  String get conversationMemoryLoadFailed =>
      'No se pudo cargar el contexto compactado. Vuelve a intentarlo.';

  @override
  String get conversationMemoryResetAction => 'Restablecer contexto';

  @override
  String get conversationMemoryResetTitle =>
      '¿Restablecer el contexto compactado?';

  @override
  String get conversationMemoryResetConfirmation =>
      'Se eliminarán el contexto compactado guardado y la medición de la última solicitud. El historial completo de mensajes y el archivo permanecerán; el contexto compactado se podrá volver a crear durante una solicitud posterior si es necesario.';

  @override
  String get conversationMemoryResetConfirm => 'Restablecer';

  @override
  String get conversationMemoryResetFailed =>
      'No se pudo restablecer el contexto compactado.';

  @override
  String get conversationMemoryUserMessage => 'Mensaje del usuario';

  @override
  String get conversationMemoryAssistantMessage => 'Respuesta del asistente';

  @override
  String get renameConversation => 'Editar título del chat';

  @override
  String get pinConversation => 'Fijar chat';

  @override
  String get unpinConversation => 'Desfijar chat';

  @override
  String get conversationTitleRequired =>
      'El título del chat no puede estar vacío.';

  @override
  String get conversationTitleSaveFailed =>
      'No se pudo guardar el título del chat.';

  @override
  String get conversationModelSaveFailed =>
      'No se pudo guardar el modelo del chat. El modelo anterior sigue seleccionado.';

  @override
  String get moreOptions => 'Más opciones';

  @override
  String get emptyChatWelcomeTitle => '¿En qué puedo ayudarte?';

  @override
  String get emptyChatWelcomeBody => 'Haz una pregunta para empezar a chatear.';

  @override
  String get switchToDarkMode => 'Cambiar al tema oscuro';

  @override
  String get switchToLightMode => 'Cambiar al tema claro';

  @override
  String get theme => 'Tema';

  @override
  String get keyboardHint =>
      'Pulsa Enter para enviar · Shift+Enter para una nueva línea';

  @override
  String get minimizeWindow => 'Minimizar ventana';

  @override
  String get maximizeWindow => 'Maximizar ventana';

  @override
  String get restoreWindow => 'Restaurar ventana';

  @override
  String get modelSelection => 'Elegir modelo';

  @override
  String get noModelConnected => 'Todavía no hay ningún modelo conectado';

  @override
  String get noModelConnectedBody =>
      'Conecta un proveedor y elige uno de sus modelos para iniciar una conversación.';

  @override
  String get close => 'Cerrar';

  @override
  String get reasoning => 'Razonamiento';

  @override
  String get reasoningMedium => 'Medio';

  @override
  String get reasoningMinimal => 'Mínimo';

  @override
  String get reasoningLow => 'Bajo';

  @override
  String get reasoningHigh => 'Alto';

  @override
  String get reasoningExtraHigh => 'Muy alto';

  @override
  String get reasoningMax => 'Máximo';

  @override
  String get reasoningUltra => 'Ultra';

  @override
  String get reasoningDefault => 'Predeterminado';

  @override
  String get reasoningDefaultHint =>
      'No se envía un nivel de razonamiento personalizado. Se usa el comportamiento predeterminado del proveedor; el nivel no se ajusta a la dificultad de la tarea.';

  @override
  String get stop => 'Detener';

  @override
  String get modelsLoading => 'Cargando modelos...';

  @override
  String get modelsUnavailable => 'Modelos no disponibles';

  @override
  String get noModelsAvailable =>
      'Actualmente no hay modelos disponibles para esta cuenta.';

  @override
  String get providerDataUnavailable =>
      'El proveedor devolvió datos que OpenChat no pudo leer. Actualiza la conexión y vuelve a intentarlo.';

  @override
  String get oauthResponseInvalid =>
      'No se pudo leer el resultado del inicio de sesión. Vuelve a conectar la cuenta.';

  @override
  String get modelCatalogUnavailable =>
      'El catálogo de modelos no está disponible. Actualiza la conexión del proveedor y vuelve a intentarlo.';

  @override
  String get messageSaveFailed =>
      'No se pudo guardar tu mensaje en el historial de chats local.';

  @override
  String get chatHistoryUnavailable =>
      'No se pudo actualizar el historial de chats. Vuelve a intentarlo.';

  @override
  String get chatRequestFailed =>
      'No se pudo completar la respuesta. Tus mensajes guardados siguen disponibles.';

  @override
  String get cachedCatalog => 'modelos en caché';

  @override
  String get attachFile => 'Adjuntar archivo';

  @override
  String get attachmentsUnavailable =>
      'Los archivos adjuntos aún no están disponibles.';

  @override
  String get removeAttachment => 'Quitar archivo adjunto';

  @override
  String get previewImage => 'Ampliar imagen';

  @override
  String get attachmentUnavailable =>
      'Este archivo adjunto no está disponible.';

  @override
  String get attachmentCountExceeded =>
      'Puedes adjuntar hasta 10 archivos y 3 imágenes.';

  @override
  String get attachmentFileTooLarge =>
      'El archivo supera el límite de tamaño permitido.';

  @override
  String get attachmentTotalTooLarge =>
      'Los archivos adjuntos no pueden superar los 14 MB en total.';

  @override
  String get unsupportedAttachmentFile =>
      'Este tipo de archivo no es compatible.';

  @override
  String get attachmentReadFailed =>
      'No se pudo leer el archivo. Selecciónalo de nuevo y vuelve a intentarlo.';

  @override
  String get attachmentSaveFailed =>
      'No se pudo guardar el archivo adjunto. Selecciónalo de nuevo y vuelve a intentarlo.';

  @override
  String get attachmentMustBeUtf8 =>
      'Los archivos de texto deben usar la codificación UTF-8.';

  @override
  String get attachmentInvalidImage =>
      'No se pudo verificar el formato del archivo de imagen.';

  @override
  String get modelDoesNotSupportImages =>
      'El modelo seleccionado no admite archivos de imagen adjuntos.';

  @override
  String get messageHint => 'Escribe un mensaje...';

  @override
  String get sendMessage => 'Enviar mensaje';

  @override
  String get send => 'Enviar';

  @override
  String get welcomeTitle => 'Iniciar una conversación';

  @override
  String get welcomeBody =>
      'Conecta un modelo de IA antes de iniciar una conversación.';

  @override
  String get historyOpen => 'Abrir historial de chats';

  @override
  String get assistantDisclaimer =>
      'Las respuestas de la IA pueden ser inexactas. Comprueba la información importante.';

  @override
  String get modelRequired => 'Conecta un modelo antes de enviar un mensaje.';

  @override
  String get selectedModelUnavailable =>
      'Este modelo ya no está disponible. Elige otro modelo.';

  @override
  String get messageModelUnavailable => 'Modelo no disponible';

  @override
  String get userMessage => 'Usuario';

  @override
  String get copyMessage => 'Copiar mensaje';

  @override
  String get messageCopied => 'Mensaje copiado al portapapeles.';

  @override
  String get messageCopyFailed =>
      'No se pudo copiar el mensaje al portapapeles.';

  @override
  String get today => 'Hoy';

  @override
  String get unavailableTime => '—:—';

  @override
  String get unavailableValue => '—';

  @override
  String responseMetadata(String rate, String tokens, String time) {
    return '$rate tok/s · $tokens tokens · $time';
  }

  @override
  String get responseCompleted => 'Respuesta completada';

  @override
  String responseCompletedWithDuration(String duration) {
    return 'Respuesta completada · $duration';
  }

  @override
  String get reasoningSummary => 'Resumen del razonamiento';

  @override
  String reasoningSummaryWithDuration(String duration) {
    return 'Resumen del razonamiento · $duration';
  }

  @override
  String get reasoningSummaryTooltip =>
      'Un resumen proporcionado por el modelo. La duración indica cuánto tardó en aparecer en el flujo; no contiene datos ocultos de cadena de pensamiento.';

  @override
  String get toolRunning => 'En ejecución';

  @override
  String get toolWaitingForUser => 'Esperando tu respuesta';

  @override
  String get toolCompleted => 'Completado';

  @override
  String get toolFailed => 'Error';

  @override
  String get toolInput => 'Entrada';

  @override
  String get toolOutput => 'Salida';

  @override
  String get toolListFiles => 'Listar archivos';

  @override
  String get toolSearchFiles => 'Buscar en archivos';

  @override
  String get toolReadFile => 'Leer archivo';

  @override
  String get toolGetFileInfo => 'Obtener información del archivo';

  @override
  String get toolWriteFile => 'Escribir en archivo';

  @override
  String get toolEditFile => 'Editar archivo';

  @override
  String get toolExecuteCommand => 'Ejecutar comando';

  @override
  String get toolSendTerminalInput => 'Enviar entrada al terminal';

  @override
  String get toolWebSearch => 'Buscar en la web';

  @override
  String get toolReadUrlContent => 'Leer página web';

  @override
  String get toolSearchQuery => 'Consulta de búsqueda';

  @override
  String get toolWebSearchNoResults => 'No se encontraron resultados web.';

  @override
  String toolWebSearchResultCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count resultados',
      one: '1 resultado',
      zero: '0 resultados',
    );
    return '$_temp0';
  }

  @override
  String get toolUrl => 'URL';

  @override
  String toolReadUrlLength(int count) {
    return '$count caracteres';
  }

  @override
  String get toolOpenUrl => 'Abrir en el navegador';

  @override
  String get toolCopyUrl => 'Copiar URL';

  @override
  String get toolCopyContent => 'Copiar contenido';

  @override
  String get toolCopyFailed => 'No se pudo copiar el contenido.';

  @override
  String get toolOperationWorking => 'Esta operación está en curso.';

  @override
  String get toolOperationFailed => 'No se pudo completar la operación.';

  @override
  String get toolOperationUnavailable => 'No se pudo mostrar el resultado.';

  @override
  String get toolOperationTruncated =>
      'Solo está disponible una parte del resultado.';

  @override
  String get toolSearchNoMatches => 'No se encontraron coincidencias.';

  @override
  String get toolSearchMoreResults => 'Hay más coincidencias disponibles.';

  @override
  String toolSearchMatchCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count coincidencias',
      one: '1 coincidencia',
      zero: '0 coincidencias',
    );
    return '$_temp0';
  }

  @override
  String get toolReadNoLines => 'No hay líneas en este intervalo.';

  @override
  String get toolReadMoreLines => 'Hay más líneas disponibles.';

  @override
  String toolReadLineCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count líneas',
      one: '1 línea',
      zero: '0 líneas',
    );
    return '$_temp0';
  }

  @override
  String get toolFileTypeFile => 'Archivo';

  @override
  String get toolFileTypeDirectory => 'Carpeta';

  @override
  String toolWriteSuccess(String size) {
    return '$size escrito';
  }

  @override
  String get toolFilePreview => 'Vista previa del contenido escrito';

  @override
  String toolEditSuccess(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Se aplicaron # cambios',
      one: 'Se aplicó # cambio',
    );
    return '$_temp0';
  }

  @override
  String get toolEditBefore => 'Antes';

  @override
  String get toolEditAfter => 'Después';

  @override
  String get toolPermissionCommand => 'Comando';

  @override
  String get toolPermissionTerminalId => 'ID del terminal';

  @override
  String get toolPermissionInput => 'Entrada del terminal';

  @override
  String get toolTerminalNoOutput => 'No se produjo ninguna salida.';

  @override
  String get toolTerminalWaitingOutput => 'Esperando salida o entrada...';

  @override
  String get toolTerminalRunning => 'En ejecución...';

  @override
  String get toolTerminalTerminated => 'Terminado';

  @override
  String get toolTerminalWaitingForInput => 'Esperando entrada';

  @override
  String toolTerminalExitCode(int code) {
    return 'Código de salida: $code';
  }

  @override
  String get toolTerminalCopied => 'Copiado al portapapeles';

  @override
  String get toolTechnicalDetails => 'Detalles';

  @override
  String toolFileCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count elementos',
      one: '1 elemento',
      zero: 'Ningún elemento',
    );
    return '$_temp0';
  }

  @override
  String get toolEmptyListing =>
      'No hay elementos que mostrar en esta carpeta.';

  @override
  String get toolListingUnavailable =>
      'No se pudo mostrar esta lista de archivos.';

  @override
  String get toolListingIncomplete =>
      'No se pudieron listar algunos elementos.';

  @override
  String get toolMoreFilesAvailable => 'Hay más elementos disponibles.';

  @override
  String get toolDesktopLocation => 'Escritorio';

  @override
  String get toolProjectLocation => 'Carpeta del proyecto';

  @override
  String get toolOpenChatLocation => 'Carpeta de datos de la aplicación';

  @override
  String get responseFailed => 'Respuesta fallida';

  @override
  String get responseStopped => 'Respuesta detenida';

  @override
  String secondsShort(int count) {
    return '$count s';
  }

  @override
  String get usageQuotas => 'Cuotas de uso';

  @override
  String get usageQuotasDescription =>
      'Consulta las cuotas restantes, los límites de uso y las horas de restablecimiento de todas tus cuentas de ChatGPT conectadas.';

  @override
  String get refreshAll => 'Actualizar todo';

  @override
  String get noChatGptAccountsForQuota =>
      'No se encontraron cuentas de ChatGPT conectadas.';

  @override
  String get noChatGptAccountsForQuotaDescription =>
      'Conecta tu cuenta de ChatGPT en la pestaña Conexiones para consultar tus límites y cuotas de uso.';

  @override
  String get goToConnections => 'Ir a Conexiones';

  @override
  String get activeAccountBadge => 'Activa';

  @override
  String workspaceQuotaLabel(String name) {
    return 'Espacio de trabajo: $name';
  }

  @override
  String get modelsPageDescription =>
      'Busca y descarga modelos alojados en Hugging Face.';

  @override
  String get modelSortDownloads => 'Más descargados';

  @override
  String get modelSortLikes => 'Más populares';

  @override
  String get modelSortRecentlyUpdated => 'Actualizados recientemente';

  @override
  String get modelPreviousPage => 'Anterior';

  @override
  String get modelNextPage => 'Siguiente';

  @override
  String modelPageLabel(int page) {
    return 'Página $page';
  }

  @override
  String get modelFormatGguf => 'GGUF · llama.cpp';

  @override
  String get modelFormatTransformers => 'Transformers · vLLM';

  @override
  String get modelFormatExllama => 'ExLlama · EXL3';

  @override
  String get huggingFaceModelSearchHint => 'Buscar modelos de Hugging Face';

  @override
  String get modelSearchRefresh => 'Actualizar resultados de modelos';

  @override
  String get modelSearchEmpty => 'Ningún modelo coincide con esta búsqueda.';

  @override
  String get modelSearchFailed =>
      'No se pudieron cargar los modelos de Hugging Face.';

  @override
  String get modelSearchUnavailable =>
      'No se pudo acceder a Hugging Face. Comprueba tu conexión y vuelve a intentarlo.';

  @override
  String get modelSearchRateLimited =>
      'Hugging Face está recibiendo demasiadas solicitudes. Espera un momento y vuelve a intentarlo.';

  @override
  String get modelSearchInvalidResponse =>
      'Hugging Face devolvió información de modelos que OpenChat no pudo leer. Vuelve a intentarlo dentro de un momento.';

  @override
  String get modelSearchTimedOut =>
      'Hugging Face tardó demasiado en responder. Vuelve a intentarlo.';

  @override
  String get modelChooseForDetails =>
      'Elige un modelo para consultar sus archivos.';

  @override
  String get modelDownloadsLabel => 'Descargas';

  @override
  String get modelLikesLabel => 'Me gusta';

  @override
  String get modelLicenseLabel => 'Licencia';

  @override
  String get modelRevisionLabel => 'Revisión';

  @override
  String get modelFilesLabel => 'Archivos del modelo';

  @override
  String get modelVisionComponentsLabel => 'Componentes de visión';

  @override
  String get modelMtpComponentsLabel => 'Componentes MTP';

  @override
  String get modelAuxiliaryComponentsLabel => 'Otros componentes auxiliares';

  @override
  String get modelDownloadComponentButton => 'Descargar este componente';

  @override
  String get modelComponentDownloaded => 'Componente descargado';

  @override
  String get modelShowMoreComponents => 'Mostrar más componentes';

  @override
  String get modelReadmeLabel => 'Descripción del modelo';

  @override
  String get modelReadmeMissing => 'Este modelo no tiene README.';

  @override
  String get modelReadmeAccessDenied =>
      'Se necesita acceso a este repositorio para ver su descripción.';

  @override
  String get modelReadmeTooLarge =>
      'El README es demasiado grande para mostrarlo.';

  @override
  String get modelReadmeUnavailable =>
      'No se pudo cargar la descripción del modelo.';

  @override
  String get modelDownloadOptionsLabel => 'Opciones de descarga';

  @override
  String get modelDownloadGroupLabel => 'Conjunto de archivos';

  @override
  String get modelDownloadSizeLabel => 'Tamaño';

  @override
  String get modelDownloadButton => 'Descargar modelo';

  @override
  String get modelCancelDownload => 'Cancelar descarga';

  @override
  String modelDownloadRunning(String fileName, int fileIndex, int fileCount) {
    return 'Archivo $fileIndex de $fileCount: $fileName';
  }

  @override
  String get modelDownloadComplete =>
      'Modelo descargado y añadido a Modelos locales.';

  @override
  String get modelDownloadCancelled =>
      'Descarga del modelo cancelada. Puedes reanudarla más tarde.';

  @override
  String get modelDownloadFailed => 'No se pudo descargar el modelo.';

  @override
  String get modelDownloadProgressUnavailable =>
      'No se pudo leer el progreso de descarga.';

  @override
  String get modelRevisionChanged =>
      'Este modelo ha cambiado en Hugging Face. Vuelve a cargar sus archivos y vuelve a intentarlo.';

  @override
  String get modelDownloadAccessNeeded =>
      'Este repositorio está restringido o es privado y requiere acceso a Hugging Face.';

  @override
  String get modelNoCompatibleFiles =>
      'No se encontraron archivos de modelo completos y compatibles en este repositorio.';

  @override
  String get modelUnknownDownloadSize =>
      'El tamaño del archivo no está disponible, por lo que la descarga no se puede iniciar de forma segura.';

  @override
  String get modelDetailsLoading => 'Cargando archivos del modelo…';

  @override
  String get modelNoFiles =>
      'No hay archivos compatibles disponibles para este formato.';

  @override
  String get modelGatedBadge => 'Acceso necesario';

  @override
  String get modelPrivateBadge => 'Privado';

  @override
  String get modelSavedToFolder =>
      'Las descargas se guardan en la carpeta seleccionada para este motor en Ajustes.';

  @override
  String get userQuestionTitle => 'El asistente espera tu respuesta';

  @override
  String get userQuestionRequiredHint =>
      'Las preguntas obligatorias están marcadas';

  @override
  String get userQuestionSubmit => 'Enviar respuesta';

  @override
  String get userQuestionResuming => 'Reanudando el asistente';

  @override
  String get userQuestionUnavailable =>
      'Esta pregunta ya no está disponible. Vuelve a cargar la conversación.';

  @override
  String get userQuestionRequiredValidation =>
      'Responde todas las preguntas obligatorias para continuar.';

  @override
  String get userQuestionSubmitFailed =>
      'No se pudo guardar tu respuesta. Inténtalo de nuevo.';

  @override
  String get userQuestionRequiredLabel => 'Obligatorio';

  @override
  String get userQuestionContinue => 'Continuar asistente';

  @override
  String get userQuestionSaved =>
      'Tu respuesta está guardada. Continúa cuando quieras.';

  @override
  String get userQuestionLoadFailed =>
      'No se pudo cargar la pregunta pendiente. Inténtalo de nuevo.';

  @override
  String get userQuestionResumeFailed =>
      'La respuesta está guardada, pero el asistente no pudo continuar. Inténtalo de nuevo.';

  @override
  String get userQuestionNotificationTitle => 'OpenChat te está esperando';

  @override
  String get userQuestionNotificationBody =>
      'La IA está esperando tu respuesta.';
}
