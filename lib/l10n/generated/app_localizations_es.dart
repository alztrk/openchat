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
  String get localEngineDeprecated => 'En desuso';

  @override
  String get localEngineWindowsDeprecatedReason =>
      'Las nuevas instalaciones de vLLM y ExLlama están desactivadas en Windows. Se conservan los archivos y los modelos ya registrados.';

  @override
  String get localEngineUnsupportedPlatform => 'Plataforma no compatible';

  @override
  String get localEngineHardwareUnavailable => 'Hardware no disponible';

  @override
  String get localEngineDriverUnsupported => 'Actualiza el controlador NVIDIA';

  @override
  String get localEngineDriverVersionUnavailable =>
      'No se pudo verificar la versión del controlador NVIDIA';

  @override
  String get localEngineVllmBlockedReason =>
      'vLLM requiere un controlador NVIDIA versión 580 o posterior y un entorno Linux existente. En Windows, usa una distribución WSL2 existente con acceso a la GPU.';

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
  String get localEngineExecutable => 'Ejecutable de llama-server';

  @override
  String get localEngineExecutableDescription =>
      'Elige un ejecutable de llama-server para que OpenChat inicie modelos GGUF registrados. Esta opción es independiente de conectarse a un servidor que hayas iniciado.';

  @override
  String get localEngineExecutableChoose => 'Elegir ejecutable';

  @override
  String get localEngineExecutableClear => 'Quitar selección';

  @override
  String get localEngineExecutableNotConfigured =>
      'No se ha elegido ningún ejecutable. OpenChat puede usar un paquete instalado.';

  @override
  String get localEngineExecutableMissing =>
      'No se encontró el ejecutable guardado. Vuelve a seleccionarlo.';

  @override
  String get localEngineExecutableInvalid =>
      'Elige un ejecutable de llama-server existente.';

  @override
  String get localEngineSettingsFailed =>
      'No se pudo guardar la configuración de llama-server.';

  @override
  String get localEngineExternalServerCheck =>
      'Buscar llama-server en ejecución';

  @override
  String get localEngineExternalServerNotConnected =>
      'No hay ningún servidor iniciado por el usuario conectado. OpenChat no iniciará ni detendrá este servidor.';

  @override
  String get localEngineExternalServerConnecting =>
      'Comprobando el servidor local seleccionado...';

  @override
  String get localEngineExternalServerFoundTitle =>
      'Se encontró un llama-server en ejecución';

  @override
  String localEngineExternalServerFoundDescription(int port) {
    return 'Hay un servidor llama.cpp escuchando en el puerto $port. OpenChat se conectará y mostrará sus modelos. Desconectarlo en OpenChat no detendrá el servidor.';
  }

  @override
  String get localEngineExternalServerNotNow => 'Ahora no';

  @override
  String get localEngineExternalServerConnect => 'Conectar';

  @override
  String localEngineExternalServerConnected(int port, int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Hay $count modelos disponibles',
      one: 'Hay 1 modelo disponible',
    );
    return 'Conectado al puerto $port. $_temp0.';
  }

  @override
  String get localEngineManagedModelSection =>
      'Modelos gestionados por OpenChat';

  @override
  String localEngineManagedServerModelSection(int port) {
    return 'Servidor de OpenChat · 127.0.0.1:$port';
  }

  @override
  String localEngineExternalModelSection(int port) {
    return 'Servidor del usuario · 127.0.0.1:$port';
  }

  @override
  String get localEngineExternalServerDisconnect => 'Desconectar';

  @override
  String get localEngineExternalServerNotFound =>
      'No se encontró ningún llama-server en ejecución.';

  @override
  String get localEngineExternalServerScanFailed =>
      'No se pudieron comprobar los procesos llama-server en ejecución.';

  @override
  String get localEngineExternalServerConnectFailed =>
      'No se pudo conectar. Comprueba que llama-server esté listo y exponga su endpoint local de modelos.';

  @override
  String get localEngineExternalServerAuthRequired =>
      'Este servidor requiere autenticación. OpenChat no lee ni reutiliza credenciales de otros procesos.';

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
  String get localModelContextUnavailable =>
      'El motor local no indicó su ventana de contexto activa. Actualiza o reinstala llama.cpp y vuelve a intentarlo.';

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
  String get localEngineStageRuntimeSetup => 'Preparando el entorno de Python';

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
  String get chatGptFastModeEnabledTooltip =>
      'Se solicita el modo Fast. Puede consumir créditos de suscripción más rápido o aumentar el coste por token de API.';

  @override
  String get chatGptFastModeDisabledTooltip =>
      'Solicitar el modo Fast. Puede consumir créditos de suscripción más rápido o aumentar el coste por token de API; la disponibilidad depende del modelo.';

  @override
  String get chatGptFastModeUnavailableTooltip =>
      'El modelo seleccionado no indica compatibilidad con el modo Fast.';

  @override
  String get chatGptFastModeLoadingTooltip =>
      'Cargando la preferencia del modo Fast…';

  @override
  String get chatGptFastModeSettingsLoadFailed =>
      'No se pudo cargar la preferencia del modo Fast.';

  @override
  String get chatGptFastModeSettingsSaveFailed =>
      'No se pudo guardar la preferencia del modo Fast.';

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
  String get archiveConversation => 'Archivar conversación';

  @override
  String get restoreConversation => 'Restaurar conversación';

  @override
  String get archivedChats => 'Archivadas';

  @override
  String get noArchivedChats => 'No hay conversaciones archivadas.';

  @override
  String get stopResponseBeforeArchive =>
      'Detén la respuesta activa antes de archivar esta conversación.';

  @override
  String get conversationArchived => 'Conversación archivada.';

  @override
  String get conversationRestored => 'Conversación restaurada.';

  @override
  String get conversationArchiveFailed =>
      'No se pudo archivar la conversación. Inténtalo de nuevo.';

  @override
  String get conversationRestoreFailed =>
      'No se pudo restaurar la conversación. Inténtalo de nuevo.';

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
  String get conversationDeletedFileChangesCleanupFailed =>
      'Se eliminó la conversación, pero no se pudieron borrar las copias de los cambios en los archivos.';

  @override
  String get conversationHistoryClearedFileChangesCleanupFailed =>
      'Se borró el historial de conversaciones, pero no se pudieron eliminar las copias de los cambios en los archivos.';

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
  String get selectedModelDoesNotSupportToolCalls =>
      'Este modelo no admite llamadas a herramientas. La configuración de acceso a herramientas no se aplica.';

  @override
  String get selectedModelToolSupportUnknown =>
      'Este modelo no informa si admite llamadas a herramientas. OpenChat intentará enviarlas; el proveedor podría rechazar la solicitud.';

  @override
  String get toolPermissionRequireApprovalDescription =>
      'Pide aprobación antes de cada llamada de archivos, web o terminal. Los archivos se limitan al proyecto y OpenChat.';

  @override
  String get toolPermissionApproveSafeOperations =>
      'Aprobar operaciones seguras';

  @override
  String get toolPermissionApproveSafeOperationsDescription =>
      'Lee archivos del proyecto y OpenChat automáticamente. Pregunta antes de cambios, web y terminal.';

  @override
  String get toolPermissionFullAccess => 'Acceso completo';

  @override
  String get toolPermissionFullAccessDescription =>
      'Sin aprobación: archivos en cualquier carpeta y acceso web y terminal sin preguntar.';

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
  String responseVersionCount(int current, int total) {
    return 'Respuesta $current de $total';
  }

  @override
  String get previousResponseVersion => 'Respuesta anterior';

  @override
  String get nextResponseVersion => 'Respuesta siguiente';

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
  String get providerToolRequestRejected =>
      'El proveedor rechazó una solicitud que incluía herramientas. Comprueba si el modelo admite herramientas o elige uno que lo indique.';

  @override
  String get providerNetworkUnavailable =>
      'No se pudo conectar con el proveedor. Comprueba tu conexión e inténtalo de nuevo.';

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
  String get conversationArchiveTitle => 'Archivos de conversaciones';

  @override
  String get conversationArchiveDescription =>
      'Crea o restaura un archivo cifrado con las conversaciones seleccionadas de este dispositivo.';

  @override
  String get conversationArchiveIncludesNotice =>
      'El archivo conserva los mensajes seleccionados, las entradas y salidas de las herramientas, los resúmenes de razonamiento, los ajustes de memoria por conversación y los archivos adjuntos. El texto de los chats puede contener información sensible o rutas locales. Se excluyen las credenciales, las cuentas vinculadas, los vínculos a proyectos y los ajustes globales.';

  @override
  String get conversationArchiveUnavailable =>
      'El servicio de archivos local o el historial de chats no está disponible.';

  @override
  String get exportConversations => 'Exportar conversaciones';

  @override
  String get importConversations => 'Importar archivo';

  @override
  String get conversationArchiveNoConversations =>
      'No hay conversaciones para exportar.';

  @override
  String get conversationArchiveSelectTitle =>
      'Elige las conversaciones que quieres exportar';

  @override
  String get conversationArchiveSearch => 'Buscar conversaciones';

  @override
  String conversationArchiveSelectedCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '# conversaciones seleccionadas',
      one: '# conversación seleccionada',
      zero: 'No hay conversaciones seleccionadas',
    );
    return '$_temp0';
  }

  @override
  String get conversationArchiveSelectAll => 'Seleccionar todas las visibles';

  @override
  String get conversationArchiveDeselectAll =>
      'Deseleccionar todas las visibles';

  @override
  String get conversationArchiveNoMatches =>
      'No hay conversaciones coincidentes.';

  @override
  String get conversationArchivePassphrase => 'Frase de contraseña del archivo';

  @override
  String get conversationArchiveConfirmPassphrase =>
      'Confirmar frase de contraseña';

  @override
  String get conversationArchivePassphraseHint => 'Al menos 12 caracteres';

  @override
  String get conversationArchivePassphraseTooShort =>
      'Usa al menos 12 caracteres.';

  @override
  String get conversationArchivePassphraseTooLong =>
      'La frase de contraseña no puede superar los 512 bytes.';

  @override
  String get conversationArchivePassphraseMismatch =>
      'Las frases de contraseña no coinciden.';

  @override
  String get conversationArchivePassphraseRecovery =>
      'Guarda la frase en un lugar seguro. OpenChat no puede recuperarla.';

  @override
  String get conversationArchiveChooseFolder =>
      'Elige dónde guardar el archivo cifrado';

  @override
  String get conversationArchiveChooseFile =>
      'Elige un archivo de conversaciones de OpenChat';

  @override
  String conversationArchiveExportSuccess(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Se creó un archivo cifrado para # conversaciones.',
      one: 'Se creó un archivo cifrado para # conversación.',
    );
    return '$_temp0';
  }

  @override
  String conversationArchiveImportSuccess(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Se importaron # conversaciones.',
      one: 'Se importó # conversación.',
      zero: 'No se importaron conversaciones.',
    );
    return '$_temp0';
  }

  @override
  String get conversationArchivePickerFailed =>
      'No se pudo abrir el selector de archivos o carpetas.';

  @override
  String get conversationArchiveInvalidFile =>
      'El archivo seleccionado no tiene una ruta válida.';

  @override
  String get conversationArchiveInvalidResponse =>
      'El servicio de archivos devolvió datos no válidos.';

  @override
  String get conversationArchiveExportFailed =>
      'No se pudo crear el archivo de conversaciones.';

  @override
  String get conversationArchiveImportFailed =>
      'No se pudo importar el archivo de conversaciones.';

  @override
  String get profileArchiveTitle =>
      'Copia de seguridad de los datos de la aplicación';

  @override
  String get profileArchiveDescription =>
      'Crea o restaura una copia cifrada de la base de datos de conversaciones y los archivos adjuntos asociados.';

  @override
  String get profileArchiveIncludesNotice =>
      'No incluye credenciales de proveedores, preferencias globales ni archivos de modelos o motores. Al restaurar, la base de datos y los adjuntos actuales se conservan en una carpeta de recuperación.';

  @override
  String get profileArchiveUnavailable =>
      'El servicio local de copias del perfil no está disponible.';

  @override
  String get profileArchiveExport => 'Hacer copia de los datos';

  @override
  String get profileArchiveRestore => 'Restaurar los datos';

  @override
  String get profileArchiveChooseFolder =>
      'Elige dónde guardar la copia cifrada del perfil';

  @override
  String get profileArchiveChooseFile =>
      'Elige una copia del perfil de OpenChat';

  @override
  String profileArchiveExportSuccess(
    String conversationCount,
    String messageCount,
    String attachmentCount,
  ) {
    return 'Copia cifrada creada: $conversationCount conversaciones, $messageCount mensajes y $attachmentCount archivos adjuntos.';
  }

  @override
  String get profileArchiveRestoreConfirmTitle =>
      '¿Reemplazar los datos de la aplicación?';

  @override
  String get profileArchiveRestoreConfirmBody =>
      'La copia seleccionada reemplazará la base de datos de conversaciones y sus archivos adjuntos la próxima vez que se inicie OpenChat. La base de datos y los adjuntos actuales se conservarán en una carpeta de recuperación. No se incluyen las credenciales, preferencias globales ni archivos de modelos o motores.';

  @override
  String get profileArchiveRestoreConfirmButton =>
      'Restaurar y cerrar OpenChat';

  @override
  String get profileArchiveRestartTitle =>
      'Cierra OpenChat para aplicar la copia';

  @override
  String profileArchiveRestoreReady(
    int conversationCount,
    int messageCount,
    int attachmentCount,
  ) {
    return 'La copia contiene $conversationCount conversaciones, $messageCount mensajes y $attachmentCount archivos adjuntos. Cierra OpenChat ahora. Los datos restaurados se comprobarán al iniciar y el perfil actual se conservará para recuperarlo.';
  }

  @override
  String get profileArchiveCloseApp => 'Cerrar OpenChat';

  @override
  String get profileArchiveCloseFailed =>
      'No se pudo cerrar OpenChat. Cierra la ventana para aplicar la copia.';

  @override
  String get profileArchiveProcessing =>
      'Se está cifrando o validando la copia del perfil. Las copias grandes pueden tardar varios minutos.';

  @override
  String get profileArchivePickerFailed =>
      'No se pudo abrir el selector de archivos o carpetas.';

  @override
  String get profileArchiveInvalidFile =>
      'El archivo seleccionado no tiene una ruta válida.';

  @override
  String get profileArchiveInvalidResponse =>
      'El servicio de copias del perfil devolvió datos no válidos.';

  @override
  String get profileArchiveExportFailed =>
      'No se pudo crear la copia cifrada del perfil.';

  @override
  String get profileArchiveRestoreFailed =>
      'No se pudo preparar la copia del perfil para restaurarla.';

  @override
  String get profileArchiveInvalidArchive =>
      'La copia no es válida o la contraseña es incorrecta.';

  @override
  String get profileArchivePassphraseInvalid =>
      'Usa una contraseña de al menos 12 caracteres y no más de 512 bytes.';

  @override
  String get profileArchiveNotFound =>
      'No se encontró la copia del perfil seleccionada.';

  @override
  String get profileArchiveConflict =>
      'Ya hay una restauración pendiente o el archivo de destino ya existe.';

  @override
  String get profileArchiveBusy =>
      'Hay otra operación de copia del perfil en curso.';

  @override
  String get profileArchiveStorageFailed =>
      'No se pudo leer, escribir o restaurar la copia del perfil de forma segura.';

  @override
  String get profileArchiveLimitExceeded =>
      'La copia del perfil supera un límite de tamaño o elementos admitido.';

  @override
  String get profileArchiveSchemaUnsupported =>
      'Esta copia se creó con una versión más reciente del esquema de OpenChat.';

  @override
  String get profileArchiveTakingLong =>
      'La copia del perfil está tardando más de lo esperado. Espera a que termine antes de volver a intentarlo.';

  @override
  String get profileArchiveOperationFailed =>
      'No se pudo completar la operación de copia del perfil.';

  @override
  String get conversationArchivePassphraseTitle => 'Desbloquear archivo';

  @override
  String get conversationArchivePreviewTitle =>
      'Revisar el contenido del archivo';

  @override
  String get conversationArchiveCreatedAt => 'Creado';

  @override
  String get conversationArchiveConversationCount => 'Conversaciones';

  @override
  String get conversationArchiveMessageCount => 'Mensajes';

  @override
  String get conversationArchiveAttachmentCount => 'Adjuntos';

  @override
  String get conversationArchiveDuplicateCount =>
      'Conversaciones que ya están en este dispositivo';

  @override
  String get conversationArchiveSkipDuplicates =>
      'Omitir las conversaciones que ya están en este dispositivo';

  @override
  String get conversationArchiveImportCopies =>
      'Importar los duplicados como copias separadas';

  @override
  String get conversationArchiveRestoreNotice =>
      'Las conversaciones restauradas no quedarán vinculadas a cuentas de proveedores ni a proyectos. Selecciona un modelo de nuevo para continuar esos chats.';

  @override
  String get conversationArchiveInvalidPassphraseOrFile =>
      'La frase de contraseña es incorrecta o el archivo no es válido.';

  @override
  String get conversationArchivePassphraseInvalid =>
      'La longitud de la frase de contraseña no es compatible.';

  @override
  String get conversationArchiveNotFound =>
      'No se encontró el archivo o la conversación seleccionados.';

  @override
  String get conversationArchiveConflict =>
      'El archivo de destino ya existe. Elige otra carpeta o resuelve primero el archivo existente.';

  @override
  String get conversationArchiveBusy =>
      'Detén las ejecuciones activas del asistente en los chats seleccionados antes de exportarlos.';

  @override
  String get conversationArchiveStorageFailed =>
      'No se pudo leer, escribir o restaurar el archivo de forma segura.';

  @override
  String get conversationArchiveLimitExceeded =>
      'El archivo supera el tamaño máximo admitido.';

  @override
  String get conversationArchiveTakingLong =>
      'La operación tarda más de lo previsto y puede seguir ejecutándose. Comprueba la lista de chats antes de volver a intentarlo.';

  @override
  String get conversationArchiveOperationFailed =>
      'La operación del archivo falló. Comprueba el archivo seleccionado y el espacio disponible e inténtalo de nuevo.';

  @override
  String get conversationArchiveProcessing =>
      'Cifrando o verificando el archivo. Los archivos grandes pueden tardar varios minutos.';

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
  String get continueLabel => 'Continuar';

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
  String get searchChatsHint => 'Buscar chats y mensajes';

  @override
  String get searchMessagesTooltip => 'Buscar mensajes';

  @override
  String get historySearchDateFilter => 'Filtrar por fecha';

  @override
  String get historySearchDateFilterApplied => 'El filtro de fecha está activo';

  @override
  String get historySearchFiltersTitle => 'Filtros de búsqueda';

  @override
  String get historySearchRouteFilterNote =>
      'El proveedor y el modelo corresponden a la ruta guardada con cada respuesta.';

  @override
  String get historySearchProviderFilter => 'Proveedor de la respuesta';

  @override
  String get historySearchModelFilter => 'Modelo de la respuesta';

  @override
  String get historySearchProjectFilter => 'Proyecto';

  @override
  String get historySearchArchiveFilter => 'Estado del archivo';

  @override
  String get historySearchAllProviders => 'Todos los proveedores';

  @override
  String get historySearchAllModels => 'Todos los modelos';

  @override
  String get historySearchAllProjects => 'Todos los proyectos';

  @override
  String get historySearchTagFilter => 'Etiqueta';

  @override
  String get selectConversations => 'Seleccionar chats';

  @override
  String get cancelSelection => 'Cancelar selección';

  @override
  String get archiveSelectedConversations => 'Archivar chats seleccionados';

  @override
  String get moveSelectedChats => 'Mover chats seleccionados';

  @override
  String get moveToChats => 'Mover a chats';

  @override
  String get bookmarkConversation => 'Guardar conversación en marcadores';

  @override
  String get removeConversationBookmark => 'Quitar marcador';

  @override
  String get conversationBookmarkFailed =>
      'No se pudo actualizar el marcador de la conversación.';

  @override
  String get conversationBookmarked => 'En marcadores';

  @override
  String selectConversation(String title) {
    return 'Seleccionar $title';
  }

  @override
  String conversationsSelected(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '# chats seleccionados',
      one: '# chat seleccionado',
      zero: 'No hay chats seleccionados',
    );
    return '$_temp0';
  }

  @override
  String get historySearchAllTags => 'Todas las etiquetas';

  @override
  String get historySearchAllStatuses => 'Todas las conversaciones';

  @override
  String get historySearchActiveConversations => 'Conversaciones activas';

  @override
  String get historySearchArchivedConversations => 'Conversaciones archivadas';

  @override
  String get historySearchClearFilters => 'Borrar filtros';

  @override
  String get historySearchApplyFilters => 'Aplicar filtros';

  @override
  String get historySearchOpenFilters => 'Abrir filtros de búsqueda';

  @override
  String get historySearchFiltersActive =>
      'Los filtros de búsqueda están activos';

  @override
  String get historySearchClearDateFilter => 'Quitar filtro de fecha';

  @override
  String get searchMessagesHeader => 'Coincidencias en mensajes';

  @override
  String get searchMessagesLoading => 'Buscando mensajes…';

  @override
  String get searchMessagesNoResults => 'No hay mensajes coincidentes';

  @override
  String get searchMessagesTooShort =>
      'Escribe al menos dos caracteres para buscar mensajes.';

  @override
  String get searchMessagesTooLong =>
      'El texto de búsqueda no puede superar los 512 caracteres.';

  @override
  String get searchMessagesFailed =>
      'No se pudieron buscar los mensajes. Inténtalo de nuevo.';

  @override
  String get searchMessageUnavailable => 'Este mensaje ya no está disponible.';

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
  String get projectToolRulesTitle => 'Permisos de herramientas del proyecto';

  @override
  String get projectToolRulesDescription =>
      'Elige una regla para cada herramienta. Las herramientas sin regla usan el permiso global. Permitir sigue respetando el alcance de archivos y el aislamiento de procesos.';

  @override
  String get projectToolRuleInherit => 'Usar el ajuste global';

  @override
  String get projectToolRuleAsk => 'Pedir aprobación';

  @override
  String get projectToolRuleAllow => 'Permitir';

  @override
  String get projectToolRuleDeny => 'Denegar';

  @override
  String get projectToolRulesLoadFailed =>
      'No se pudieron cargar los permisos de herramientas del proyecto. Revisa los ajustes guardados antes de enviar otra solicitud.';

  @override
  String get projectToolRulesSaveFailed =>
      'No se pudieron guardar los permisos de herramientas del proyecto. Las reglas anteriores siguen activas.';

  @override
  String get projectOptionsTitle => 'Opciones del proyecto';

  @override
  String get projectOptionsToolPermissions => 'Permisos de herramientas';

  @override
  String get projectOptionsMcpServers => 'Servidores MCP';

  @override
  String get projectOptionsWorktrees => 'Worktrees de Git';

  @override
  String get projectMcpTitle => 'Servidores MCP del proyecto';

  @override
  String get projectMcpDescription =>
      'Configura servidores MCP locales por stdio para este proyecto. Los servidores habilitados solo se inician cuando un chat compatible con herramientas necesita sus herramientas.';

  @override
  String get projectMcpPermissionsLocal =>
      'Los permisos se guardan localmente para este proyecto y no se almacenan en el repositorio.';

  @override
  String get projectMcpPermissionScope => 'Permiso para este servidor';

  @override
  String get projectMcpNoCredentials =>
      'No incluyas credenciales en los argumentos. Añade nombres de variables de entorno y guarda sus valores de forma segura con el botón de llave.';

  @override
  String get projectMcpEmpty =>
      'No hay servidores MCP configurados para este proyecto.';

  @override
  String get projectMcpAdd => 'Añadir servidor';

  @override
  String get projectMcpEdit => 'Editar servidor';

  @override
  String get projectMcpRemove => 'Quitar servidor';

  @override
  String get projectMcpServerId => 'ID del servidor';

  @override
  String get projectMcpProgram => 'Ruta absoluta del programa';

  @override
  String get projectMcpArguments => 'Argumentos';

  @override
  String get projectMcpArgumentsHint =>
      'Introduce un argumento por línea. Las líneas vacías se ignoran.';

  @override
  String get projectMcpEnabled => 'Habilitado';

  @override
  String get projectMcpCheck => 'Comprobar conexión';

  @override
  String get projectMcpChecking => 'Comprobando conexión…';

  @override
  String projectMcpConnected(int count) {
    return 'Conectado; se encontraron $count herramientas';
  }

  @override
  String get projectMcpCheckFailed =>
      'No se pudo iniciar el servidor ni obtener su lista de herramientas.';

  @override
  String get projectMcpLoadFailed =>
      'No se pudo cargar el catálogo MCP del proyecto. Comprueba si `.openchat/mcp.json` contiene entradas no válidas o inseguras.';

  @override
  String get projectMcpSaveFailed =>
      'No se pudo guardar el catálogo MCP. Comprueba los ID de servidor y las rutas absolutas del programa.';

  @override
  String get projectMcpDuplicateId => 'Cada servidor debe tener un ID único.';

  @override
  String get projectMcpInvalidId =>
      'Usa de 1 a 24 letras minúsculas, números o guiones bajos.';

  @override
  String get projectMcpProgramRequired =>
      'Introduce una ruta absoluta al programa del servidor.';

  @override
  String get projectMcpTransport => 'Tipo de conexión';

  @override
  String get projectMcpTransportStdio => 'Proceso local (stdio)';

  @override
  String get projectMcpTransportHttp => 'Servidor remoto (Streamable HTTP)';

  @override
  String get projectMcpEndpoint => 'Punto de conexión HTTPS';

  @override
  String get projectMcpEndpointHint =>
      'Usa el punto HTTPS del servidor MCP remoto. Se bloquean las direcciones de redes privadas y locales.';

  @override
  String get projectMcpEndpointRequired =>
      'Introduce un punto HTTPS público válido.';

  @override
  String get projectMcpAuthVariable => 'Nombre de la credencial segura';

  @override
  String get projectMcpAuthVariableHint =>
      'Opcional. Guarda su valor con el botón de llave después de añadir el servidor.';

  @override
  String get projectMcpEnvironmentVariables =>
      'Variables de entorno de credenciales';

  @override
  String get projectMcpEnvironmentVariablesHint =>
      'Introduce un nombre de variable por línea. Guarda el servidor y usa después el botón de llave para guardar los valores de forma segura.';

  @override
  String get projectMcpEnvironmentVariablesInvalid =>
      'Introduce hasta 32 nombres de variable únicos con letras, números y guiones bajos.';

  @override
  String get projectMcpCredentials => 'Credenciales MCP';

  @override
  String get projectMcpEnvironmentVariablesRequired =>
      'Edita el servidor y añade nombres de variables antes de guardar credenciales.';

  @override
  String get projectMcpCredentialHint =>
      'Los valores se guardan en el Administrador de credenciales de Windows y solo se pasan a este proceso del servidor.';

  @override
  String get projectMcpSecret => 'Valor secreto';

  @override
  String get projectMcpCredentialStored =>
      'Hay un valor guardado de forma segura.';

  @override
  String get projectMcpCredentialMissing => 'No hay ningún valor guardado.';

  @override
  String get projectMcpCredentialSave => 'Guardar de forma segura';

  @override
  String get projectMcpCredentialRemove => 'Eliminar valor guardado';

  @override
  String get projectMcpCredentialUnavailable =>
      'No se pudo acceder a la credencial. Comprueba el Administrador de credenciales de Windows e inténtalo de nuevo.';

  @override
  String get projectMcpSecretRequired => 'Introduce un valor antes de guardar.';

  @override
  String get projectMcpSecretTooLarge =>
      'El valor debe ocupar 2.500 bytes o menos.';

  @override
  String get projectWorktreesTitle => 'Worktrees aislados';

  @override
  String get projectWorktreesDescription =>
      'Crea una rama desde el commit actual en una carpeta independiente. No se copian los cambios sin confirmar.';

  @override
  String get projectWorktreesLoading => 'Cargando worktrees…';

  @override
  String get projectWorktreesEmpty =>
      'Aún no se han creado worktrees para este proyecto.';

  @override
  String get projectWorktreeCreate => 'Crear worktree';

  @override
  String get projectWorktreeCreateFailed =>
      'No se pudo crear el worktree. Comprueba que esta carpeta sea un repositorio de Git.';

  @override
  String get projectWorktreeLoadFailed =>
      'No se pudieron cargar los worktrees del proyecto.';

  @override
  String get projectWorktreeOperationFailed =>
      'La operación de Git falló. Comprueba el estado del repositorio e inténtalo de nuevo.';

  @override
  String get projectWorktreeNotRepository =>
      'Esta carpeta del proyecto no está dentro de un repositorio de Git.';

  @override
  String projectWorktreeBranch(String branch) {
    return 'Rama: $branch';
  }

  @override
  String projectWorktreePath(String path) {
    return 'Carpeta: $path';
  }

  @override
  String get projectWorktreeStatusClean => 'No hay cambios sin confirmar';

  @override
  String projectWorktreeStatusChanges(int count) {
    return 'Archivos modificados: $count';
  }

  @override
  String get projectWorktreeReview => 'Revisar cambios';

  @override
  String get projectWorktreeUse => 'Usar como proyecto';

  @override
  String get projectWorktreeRemove => 'Eliminar worktree';

  @override
  String get projectWorktreeRemoveTitle => '¿Eliminar worktree?';

  @override
  String projectWorktreeRemoveDescription(String branch) {
    return 'Se descartarán los archivos sin confirmar y sin seguimiento de $branch. La rama y sus commits se conservarán.';
  }

  @override
  String get projectWorktreeReviewTitle => 'Cambios del worktree';

  @override
  String get projectWorktreeNoChanges =>
      'No hay cambios sin confirmar. Los commits están en esta rama.';

  @override
  String get projectWorktreeStagedDiff => 'Cambios preparados';

  @override
  String get projectWorktreeUnstagedDiff => 'Cambios sin preparar';

  @override
  String get projectWorktreeListTruncated =>
      'Solo se muestran los primeros 20 worktrees.';

  @override
  String get projectWorktreeCheckFailed =>
      'No se pudo inspeccionar el worktree.';

  @override
  String get projectWorktreeRunCheck => 'Ejecutar comprobación';

  @override
  String get projectWorktreeTaskPickerTitle =>
      'Elegir una comprobación con nombre';

  @override
  String get projectWorktreeTaskEmpty =>
      'Este worktree no tiene comprobaciones con nombre. Añade tareas en `.openchat/tasks.json` del proyecto.';

  @override
  String get projectWorktreeTaskLoadFailed =>
      'No se pudieron cargar las comprobaciones con nombre de este worktree.';

  @override
  String get projectWorktreeTaskConfirmationTitle =>
      '¿Ejecutar esta comprobación?';

  @override
  String get projectWorktreeTaskCommand => 'Comando';

  @override
  String projectWorktreeTaskTimeout(int seconds) {
    return 'Límite de tiempo: $seconds segundos';
  }

  @override
  String get projectWorktreeTaskRun => 'Ejecutar comprobación';

  @override
  String projectWorktreeTaskRunning(String task) {
    return 'Comprobación en curso: $task';
  }

  @override
  String get projectWorktreeTaskStopping => 'Deteniendo comprobación…';

  @override
  String get projectWorktreeTaskStop => 'Detener comprobación';

  @override
  String get projectWorktreeTaskCancelled => 'La comprobación se canceló.';

  @override
  String get projectWorktreeTaskTimedOut =>
      'La comprobación alcanzó su límite de tiempo.';

  @override
  String projectWorktreeTaskExitCode(int code) {
    return 'La comprobación terminó con el código $code.';
  }

  @override
  String get projectWorktreeTaskExitCodeUnavailable =>
      'La comprobación terminó sin un código de salida.';

  @override
  String get projectWorktreeTaskOutputTruncated =>
      'La salida está limitada a los primeros 128 KiB.';

  @override
  String get projectWorktreeTaskRunFailed =>
      'No se pudo ejecutar la comprobación en el espacio aislado del worktree.';

  @override
  String projectWorktreeTaskResultTitle(String task) {
    return 'Resultado de la comprobación: $task';
  }

  @override
  String get projectWorktreeTaskOutput => 'Salida';

  @override
  String get projectWorktreeTaskNoOutput =>
      'La comprobación no produjo salida.';

  @override
  String get projectWorktreeTaskDenied =>
      'Los permisos del proyecto impiden ejecutar comprobaciones con nombre.';

  @override
  String get agentRunManagerTitle => 'Ejecuciones';

  @override
  String get agentRunManagerDescription =>
      'Consulta el trabajo activo, pausado o interrumpido en tus conversaciones.';

  @override
  String get agentRunLoading => 'Cargando ejecuciones';

  @override
  String get agentRunLoadFailed =>
      'No se pudo cargar el estado de las ejecuciones.';

  @override
  String get agentRunEmpty =>
      'No hay ejecuciones activas, pausadas ni interrumpidas.';

  @override
  String get agentRunRefresh => 'Actualizar lista';

  @override
  String get agentRunOpenConversation => 'Abrir chat';

  @override
  String get agentRunStatusRunning => 'En curso';

  @override
  String get agentRunStatusPaused => 'En pausa';

  @override
  String get agentRunStatusInterrupted => 'Interrumpida';

  @override
  String get agentRunStatusUnavailable => 'Estado no disponible';

  @override
  String get agentRunStatusCompleted => 'Completada';

  @override
  String get agentRunStatusFailed => 'Fallida';

  @override
  String get agentRunStatusCancelled => 'Cancelada';

  @override
  String get agentRunSubagent => 'Ejecución secundaria';

  @override
  String agentRunSubagentTask(String objective) {
    return 'Tarea secundaria: $objective';
  }

  @override
  String get agentRunLiveStarting => 'Iniciando el análisis delegado…';

  @override
  String get agentRunLiveThinking => 'Analizando la tarea delegada…';

  @override
  String agentRunLiveUsingTool(String tool) {
    return 'Usando $tool';
  }

  @override
  String get agentRunEndedInAnotherChat =>
      'Terminó una ejecución en otro chat.';

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
  String get conversationMemoryArchiveSettingsTitle => 'Indexación del archivo';

  @override
  String get conversationMemoryArchiveSettingsDescription =>
      'Elige qué texto guardado de la conversación puede añadirse a la búsqueda local del archivo. Al desactivar una opción, se eliminan sus índices derivados de búsqueda y semánticos; la conversación original se conserva.';

  @override
  String get conversationMemoryArchiveIncludeConversation =>
      'Incluir esta conversación';

  @override
  String get conversationMemoryArchiveConversationIncluded =>
      'Los mensajes y los resultados de herramientas incluidos pueden aparecer en la búsqueda del archivo.';

  @override
  String get conversationMemoryArchiveConversationExcluded =>
      'Se eliminan los índices derivados de esta conversación y se excluye de futuras indexaciones.';

  @override
  String get conversationMemoryArchiveToolsTitle =>
      'Resultados de herramientas';

  @override
  String get conversationMemoryArchiveToolIncluded =>
      'Los detalles guardados de esta herramienta pueden aparecer en la búsqueda del archivo.';

  @override
  String get conversationMemoryArchiveToolExcluded =>
      'Los detalles guardados de esta herramienta se eliminan de los índices del archivo.';

  @override
  String get conversationMemoryArchiveNoTools =>
      'No hay resultados de herramientas completados guardados en esta conversación.';

  @override
  String get conversationMemoryArchiveSettingsSaveFailed =>
      'No se pudieron guardar los ajustes de indexación del archivo. Inténtalo de nuevo.';

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
  String get editConversationTags => 'Editar etiquetas';

  @override
  String get conversationTagsDialogTitle => 'Etiquetas de la conversación';

  @override
  String get conversationTagsFieldLabel => 'Etiquetas';

  @override
  String get conversationTagsFieldHint => 'Separa las etiquetas con comas';

  @override
  String get conversationTagsHelp =>
      'Usa hasta 12 etiquetas de 32 caracteres cada una.';

  @override
  String get conversationTagsSaveFailed =>
      'No se pudieron guardar las etiquetas.';

  @override
  String get saveHistorySearchTitle => 'Guardar búsqueda del historial';

  @override
  String get savedHistorySearchName => 'Nombre de la búsqueda';

  @override
  String get savedHistorySearchesTitle => 'Búsquedas guardadas';

  @override
  String get savedHistorySearchesEmpty => 'Todavía no hay búsquedas guardadas.';

  @override
  String get deleteSavedHistorySearch => 'Eliminar búsqueda guardada';

  @override
  String get saveCurrentHistorySearch => 'Guardar búsqueda actual';

  @override
  String get savedHistorySearchesLoadFailed =>
      'No se pudieron cargar las búsquedas guardadas.';

  @override
  String get savedHistorySearchSaveFailed =>
      'No se pudieron actualizar las búsquedas guardadas.';

  @override
  String get savedHistorySearchLimitReached =>
      'Puedes guardar hasta 20 búsquedas.';

  @override
  String get pinConversation => 'Fijar chat';

  @override
  String get unpinConversation => 'Desfijar chat';

  @override
  String get conversationTitleRequired =>
      'El título del chat no puede estar vacío.';

  @override
  String get conversationBranchEditTitle => 'Editar mensaje e iniciar una rama';

  @override
  String get conversationBranchEditLabel => 'Mensaje';

  @override
  String get conversationBranchStart => 'Iniciar rama';

  @override
  String conversationBranchTitle(String title) {
    return '$title (rama)';
  }

  @override
  String get conversationBranchCreateFailed =>
      'No se pudo crear la rama del mensaje.';

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
  String get goalAlreadyActive =>
      'Reanuda o detén el objetivo activo antes de iniciar otro en este chat.';

  @override
  String get goalStateUnavailable =>
      'OpenChat no pudo comprobar si este chat ya tiene un objetivo activo. Inténtalo de nuevo.';

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
  String responseTokenRate(String rate) {
    return '$rate tok/s';
  }

  @override
  String responseTokenCount(String count) {
    return '$count tokens';
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
  String get toolRunProjectTask => 'Ejecutar tarea del proyecto';

  @override
  String get toolDelegateTask => 'Delegar análisis';

  @override
  String get toolPermissionTask => 'Tarea';

  @override
  String get toolPermissionTimeout => 'Tiempo límite (segundos)';

  @override
  String get toolSendTerminalInput => 'Enviar entrada al terminal';

  @override
  String get toolGitStatus => 'Estado de Git';

  @override
  String get toolGitDiff => 'Diff de Git';

  @override
  String get toolGitHistory => 'Historial de Git';

  @override
  String get toolGitBranch => 'Rama';

  @override
  String get toolGitUpstream => 'Rama remota';

  @override
  String get toolGitAhead => 'Por delante';

  @override
  String get toolGitBehind => 'Por detrás';

  @override
  String get toolGitStaged => 'Preparados';

  @override
  String get toolGitUnstaged => 'Sin preparar';

  @override
  String get toolGitNoChanges => 'El árbol de trabajo está limpio.';

  @override
  String get toolGitNoDiff => 'No hay diferencias que mostrar.';

  @override
  String get toolGitNoHistory => 'No se encontraron commits.';

  @override
  String get toolWebSearch => 'Buscar en la web';

  @override
  String get toolReadUrlContent => 'Leer página web';

  @override
  String get toolSearchQuery => 'Consulta de búsqueda';

  @override
  String get toolLocalWebSource => 'Búsqueda web local de OpenChat';

  @override
  String get toolLocalPageSource => 'Lectura local de página de OpenChat';

  @override
  String get toolProviderSource => 'Fuente del proveedor';

  @override
  String toolSourceRetrievedAt(String time) {
    return 'Consultado: $time';
  }

  @override
  String get toolSourceDetails => 'Detalles de la fuente';

  @override
  String toolCitationSource(String sourceId) {
    return 'Fuente $sourceId';
  }

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
  String get statistics => 'Estadísticas';

  @override
  String get statisticsDescription =>
      'Consulta el uso de tokens, las solicitudes y el historial de cuotas de ChatGPT por proveedor, modelo y conversación.';

  @override
  String get statisticsLoadFailed =>
      'No se pudieron cargar las estadísticas de uso.';

  @override
  String get statisticsUnavailable =>
      'Las estadísticas de uso locales no están disponibles ahora.';

  @override
  String get statisticsRetry => 'Volver a cargar';

  @override
  String get statisticsDateRange => 'Rango de fechas';

  @override
  String get statisticsProvider => 'Proveedor';

  @override
  String get statisticsRunId => 'ID de ejecución';

  @override
  String statisticsRunIdValue(String runId) {
    return 'ID de ejecución: $runId';
  }

  @override
  String get statisticsModel => 'Modelo';

  @override
  String get statisticsOperation => 'Tipo de operación';

  @override
  String get statisticsReasoningEffort => 'Nivel de razonamiento';

  @override
  String get statisticsFastModeFilter => 'Modo rápido';

  @override
  String get statisticsAll => 'Todos';

  @override
  String get statisticsUnspecified => 'Sin especificar';

  @override
  String get statisticsClearFilters => 'Borrar filtros';

  @override
  String get statisticsTotalTokens => 'Tokens totales';

  @override
  String get statisticsInputTokens => 'Tokens de entrada';

  @override
  String get statisticsOutputTokens => 'Tokens de salida';

  @override
  String get statisticsReasoningTokens => 'Tokens de razonamiento';

  @override
  String get statisticsCachedInputTokens => 'Tokens de entrada en caché';

  @override
  String get statisticsCacheWriteTokens => 'Tokens escritos en caché';

  @override
  String get statisticsRequests => 'Solicitudes';

  @override
  String get statisticsSuccessfulRequests => 'Correctas';

  @override
  String get statisticsFailedRequests => 'Fallidas';

  @override
  String get statisticsCancelledRequests => 'Detenidas';

  @override
  String get statisticsInterruptedRequests => 'Interrumpidas';

  @override
  String get statisticsPendingRequests => 'En curso';

  @override
  String get statisticsConversations => 'Conversaciones';

  @override
  String get statisticsCoverage => 'Cobertura de datos de uso';

  @override
  String get statisticsInputCoverage =>
      'Solicitudes con tokens de entrada informados';

  @override
  String get statisticsOutputCoverage =>
      'Solicitudes con tokens de salida informados';

  @override
  String get statisticsReasoningCoverage =>
      'Solicitudes con tokens de razonamiento informados';

  @override
  String statisticsCoverageText(int total, int reported) {
    return 'El proveedor devolvió el uso de tokens en $reported de $total solicitudes.';
  }

  @override
  String get statisticsProviderReportedCost =>
      'Coste informado por el proveedor';

  @override
  String statisticsCostCoverage(int reported) {
    return 'Hay datos de coste para $reported solicitudes.';
  }

  @override
  String get statisticsModelsDevCatalogCost =>
      'Coste según los precios de lista de models.dev';

  @override
  String statisticsModelsDevCatalogCostCoverage(int priced) {
    return 'Equivalente al precio de lista calculado para $priced solicitudes.';
  }

  @override
  String statisticsModelsDevPricingCurrent(String date) {
    return 'Catálogo de precios de models.dev obtenido el $date.';
  }

  @override
  String statisticsModelsDevPricingStale(String date) {
    return 'models.dev no está disponible; se usan los precios guardados del $date.';
  }

  @override
  String get statisticsModelsDevPricingUnavailable =>
      'Los precios de models.dev no están disponibles. No se calculan costes sin una coincidencia exacta de proveedor y modelo.';

  @override
  String get statisticsUsageTrend => 'Uso a lo largo del tiempo';

  @override
  String get statisticsDaily => 'Diario';

  @override
  String get statisticsMonthly => 'Mensual';

  @override
  String get statisticsProviders => 'Uso por proveedor';

  @override
  String get statisticsModels => 'Uso por modelo';

  @override
  String get statisticsReasoningLevels => 'Nivel de razonamiento seleccionado';

  @override
  String get statisticsOperations => 'Tipos de operación';

  @override
  String get statisticsFastModeUsage => 'Uso del modo rápido';

  @override
  String get statisticsRequested => 'Solicitado';

  @override
  String get statisticsNotRequested => 'No solicitado';

  @override
  String get statisticsServiceTiers => 'Niveles de servicio devueltos';

  @override
  String get statisticsServiceTier => 'Nivel de servicio';

  @override
  String get statisticsNoBreakdownData =>
      'No hay datos para mostrar en este periodo.';

  @override
  String get statisticsNoConversationData =>
      'No hay uso por conversación en este periodo.';

  @override
  String get statisticsOpenConversation => 'Abrir conversación';

  @override
  String get statisticsRequestDetails => 'Historial de solicitudes';

  @override
  String get statisticsRequestPayload => 'Datos de la solicitud enviada';

  @override
  String get statisticsRequestContextUnavailable =>
      'El resumen de la solicitud enviada no está disponible.';

  @override
  String statisticsRequestSourceMessages(String ids) {
    return 'Identificadores de mensajes incluidos: $ids';
  }

  @override
  String statisticsRequestArchivedMessages(String ids) {
    return 'Identificadores de mensajes recuperados: $ids';
  }

  @override
  String statisticsRequestSummaryBoundary(String id) {
    return 'El resumen incluye mensajes hasta: $id';
  }

  @override
  String statisticsRequestSourceAttachments(String files) {
    return 'Archivos adjuntos enviados: $files';
  }

  @override
  String get statisticsRequestSourcesTruncated =>
      'Este registro omite algunos detalles de las fuentes.';

  @override
  String statisticsRequestMessageCount(int count) {
    return 'Mensajes enviados: $count';
  }

  @override
  String statisticsRequestImageCount(int count) {
    return 'Imágenes enviadas: $count';
  }

  @override
  String statisticsRequestToolResultCount(int count) {
    return 'Resultados de herramientas enviados: $count';
  }

  @override
  String statisticsRequestInstructionBytes(int count) {
    return 'Tamaño de instrucciones: $count bytes';
  }

  @override
  String statisticsRequestRoles(String roles) {
    return 'Roles de mensajes: $roles';
  }

  @override
  String statisticsRequestTools(String names) {
    return 'Definiciones de herramientas: $names';
  }

  @override
  String statisticsRequestCacheControls(String names) {
    return 'Controles de caché: $names';
  }

  @override
  String get statisticsNone => 'Ninguno';

  @override
  String get statisticsRequestTime => 'Hora de la solicitud';

  @override
  String get statisticsConversationTitle => 'Título de la conversación';

  @override
  String get statisticsStatus => 'Estado';

  @override
  String get statisticsUsageSource => 'Origen de los datos de uso';

  @override
  String get statisticsNoRequestData =>
      'Ninguna solicitud coincide con estos filtros.';

  @override
  String statisticsShowingRows(int start, int end, int total) {
    return '$start - $end de $total';
  }

  @override
  String get statisticsExportCsv => 'Exportar CSV';

  @override
  String get statisticsExporting => 'Exportando';

  @override
  String get statisticsExported =>
      'Las estadísticas se exportaron a un archivo CSV.';

  @override
  String get statisticsExportFailed =>
      'No se pudieron exportar las estadísticas.';

  @override
  String get statisticsQuotaHistory => 'Historial de cuotas de ChatGPT';

  @override
  String get statisticsQuotaSnapshot => 'Registro de cuota';

  @override
  String get statisticsNoQuotaHistory =>
      'No hay registros de cuotas de ChatGPT en este rango de fechas.';

  @override
  String get statisticsUsed => 'Usado';

  @override
  String get statisticsResetAt => 'Se restablece';

  @override
  String get statisticsQuotaAllowed => 'Solicitudes permitidas';

  @override
  String get statisticsQuotaBlocked => 'Solicitudes bloqueadas';

  @override
  String get statisticsQuotaUnknown => 'Estado de uso desconocido';

  @override
  String get statisticsQuotaFreshnessCurrent => 'Datos actuales';

  @override
  String get statisticsQuotaFreshnessStale => 'Datos obsoletos';

  @override
  String get statisticsQuotaFreshnessUnknown =>
      'Estado de los datos desconocido';

  @override
  String get statisticsNotReported => 'No informado';

  @override
  String get statisticsLegacyDataNote =>
      'Los mensajes anteriores pueden incluir solo tokens de salida; no se infieren los datos que faltan del modelo ni de los tokens de entrada.';

  @override
  String get statisticsModelsDevPricingNote =>
      'Los equivalentes de precio de catálogo usan los precios actuales de models.dev y no son facturas del proveedor. Se excluyen las suscripciones de ChatGPT OAuth, las solicitudes Fast y los niveles de servicio no compatibles.';

  @override
  String get statisticsLegacyOutput =>
      'Tokens de salida de mensajes anteriores';

  @override
  String get statisticsChatGptOAuth => 'ChatGPT OAuth';

  @override
  String get statisticsChatGptApi => 'ChatGPT API';

  @override
  String get statisticsOperationChat => 'Chat';

  @override
  String get statisticsOperationToolFollowUp => 'Continuación de herramienta';

  @override
  String get statisticsOperationCompaction => 'Compactación del contexto';

  @override
  String get statisticsOperationTitleGeneration =>
      'Generación del título de conversación';

  @override
  String get statisticsOperationLegacy => 'Mensaje anterior';

  @override
  String get statisticsStatusCompleted => 'Completada';

  @override
  String get statisticsStatusFailed => 'Fallida';

  @override
  String get statisticsStatusCancelled => 'Detenida';

  @override
  String get statisticsStatusInterrupted => 'Interrumpida';

  @override
  String get statisticsStatusPending => 'En curso';

  @override
  String get statisticsStatusLegacy => 'Datos históricos';

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

  @override
  String get assistantResponseNotificationReplyEmpty =>
      'Escribe una respuesta antes de enviarla.';

  @override
  String get assistantResponseNotificationReplyTooLong =>
      'Esta respuesta es demasiado larga para enviarla desde una notificación. Usa el campo de mensaje del chat.';

  @override
  String get assistantResponseNotificationReplyUnavailable =>
      'La notificación ya no está actualizada o el chat no está disponible. Abre el chat y envía un mensaje nuevo.';

  @override
  String get assistantResponseNotificationReplyNotSent =>
      'No se pudo enviar la respuesta rápida. Revisa el chat e inténtalo de nuevo.';

  @override
  String fileChangesSummary(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '# archivos cambiados',
      one: '# archivo cambiado',
    );
    return '$_temp0';
  }

  @override
  String fileChangesLineCounts(int added, int removed) {
    return '+$added / -$removed';
  }

  @override
  String get fileChangesSomeCountsUnavailable =>
      'No se pudo calcular el número de líneas';

  @override
  String get fileChangesView => 'Ver cambios';

  @override
  String get fileChangesTrackingFailed =>
      'No se pudieron registrar algunos cambios. La lista podría estar incompleta.';

  @override
  String get fileChangesTitle => 'Cambios en esta conversación';

  @override
  String get fileChangesOpenButton => 'Cambios';

  @override
  String get fileChangesLoadFailed =>
      'No se pudieron cargar los cambios de la conversación.';

  @override
  String get fileChangesDiffFailed =>
      'No se pudo cargar la diferencia del archivo.';

  @override
  String get fileChangesConflict =>
      'El archivo cambió después de la edición de la IA. No se modificó.';

  @override
  String get fileChangesRevertFailed => 'No se pudo revertir el cambio.';

  @override
  String get fileChangesEmpty =>
      'No se registraron cambios de archivos en esta conversación.';

  @override
  String get fileChangesDiffTitle =>
      'Selecciona un archivo para ver su diferencia';

  @override
  String get fileChangesBinary =>
      'Los cambios en archivos binarios no se pueden mostrar como texto.';

  @override
  String get fileChangesDiffUnavailable =>
      'No hay una diferencia de texto disponible para este archivo.';

  @override
  String get fileChangesDiffTruncated =>
      'La diferencia es larga. Solo se muestra el comienzo.';

  @override
  String get fileChangesActive => 'Modificado';

  @override
  String get fileChangesReverted => 'Revertido';

  @override
  String get fileChangesRevert => 'Revertir';

  @override
  String fileChangesMoreFiles(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '# archivos más',
      one: '# archivo más',
    );
    return '$_temp0';
  }

  @override
  String get fileChangesUnavailableTitle => 'Cambios no disponibles';

  @override
  String get goalSlashCommand => '/goal';

  @override
  String get goalCommandDescription =>
      'Inicia este mensaje como objetivo y sigue trabajando hasta completarlo o necesitar tu ayuda.';

  @override
  String get goalObjectiveRequired => 'Escribe un objetivo después de /goal.';

  @override
  String get goalWorking => 'Trabajando en el objetivo';

  @override
  String get goalPaused => 'Objetivo en pausa';

  @override
  String get goalInterrupted => 'Objetivo interrumpido';

  @override
  String get goalCompleted => 'Objetivo completado';

  @override
  String get goalStopped => 'Objetivo detenido';

  @override
  String get goalFailed => 'El objetivo ha fallado';

  @override
  String get goalPausedForQuota =>
      'En pausa por alcanzar la cuota o el límite de solicitudes del modelo.';

  @override
  String get goalPausedForBlocker => 'En pausa por un impedimento.';

  @override
  String get goalPausedForUserInput => 'En pausa mientras espera tu respuesta.';

  @override
  String get goalPausedByUser => 'Pausado por ti.';

  @override
  String get goalPausedAfterError => 'En pausa tras un error de solicitud.';

  @override
  String get goalPauseAction => 'Pausar objetivo';

  @override
  String get goalResumeAction => 'Reanudar objetivo';

  @override
  String get goalStopAction => 'Detener objetivo';

  @override
  String goalElapsedTime(String time) {
    return 'Tiempo transcurrido: $time';
  }
}
