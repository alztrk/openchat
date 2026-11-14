import { LoaderCircle, Plus, RefreshCw, Trash2 } from 'lucide-react';
import { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Dialog } from '@shared/components';
import {
  type CompatibleProviderId,
  type CustomProviderSummary,
  deleteChatGptApiKey,
  deleteCompatibleProviderApiKey,
  deleteCustomProvider,
  deleteOpenCodeApiKey,
  loadProviderCredentialStatus,
  type ProviderCredentialSnapshot,
  saveChatGptApiKey,
  saveCompatibleProviderApiKey,
  saveCustomProvider as saveCustomProviderRecord,
  saveOpenCodeApiKey,
} from '@features/provider-settings/data';
import {
  clearProviderRequestOutcomeRecord,
  type ProviderRequestRoute,
  readProviderRequestOutcomeRecord,
} from '@features/provider-settings/data';
import {
  clearProviderVerificationRecord,
  type ProviderVerificationRoute,
  readProviderVerificationRecord,
} from '@features/provider-settings/data/providerVerification';
import {
  CredentialCard,
  type CredentialFeedback,
  CredentialInput,
  CustomProviderCard,
  FeedbackMessage,
  type ProviderRequestOutcomeView,
  SavedChatGptApiKey,
} from './ApiKeyProviderCards';
import { CustomProviderDialog, type CustomProviderDraft } from './CustomProviderDialog';
import {
  notCheckedVerificationState,
  type ProviderVerificationViewState,
  providerVerificationStateForOutcome,
  requestOutcomeEntries,
  verificationEntries,
} from './ProviderVerificationStatus';

interface ApiKeyProvidersSectionProps {
  mode: 'chatgpt' | 'modelProviders';
  onChanged(): Promise<void>;
  chatGptApiKeyFormOpen?: boolean;
  onChatGptApiKeyFormToggle?(): void;
}

type CompatibleProviderNameKey =
  | 'providerNameGemini'
  | 'providerNameGroq'
  | 'providerNameCerebras'
  | 'providerNameOpenRouter'
  | 'providerNameMistral'
  | 'providerNameDeepSeek'
  | 'providerNameXai'
  | 'providerNamePerplexity'
  | 'providerNameTogether'
  | 'providerNameSambaNova'
  | 'providerNameAnthropic'
  | 'providerNameFireworks'
  | 'providerNameDeepInfra'
  | 'providerNameDashScope'
  | 'providerNameCohere'
  | 'providerNameNvidia'
  | 'providerNameKilo'
  | 'providerNameMoonshot'
  | 'providerNameZai'
  | 'providerNameMinimax'
  | 'providerNameAi21'
  | 'providerNameUpstage'
  | 'providerNameHyperbolic'
  | 'providerNameNovita'
  | 'providerNameSiliconFlow'
  | 'providerNameFeatherless'
  | 'providerNameNebius'
  | 'providerNameScaleway'
  | 'providerNameStepFun'
  | 'providerNameYi'
  | 'providerNameBaichuan'
  | 'providerNameVolcengine'
  | 'providerNameHunyuan'
  | 'providerNameQianfan'
  | 'providerNameInflection'
  | 'providerNameInception'
  | 'providerNameMimo'
  | 'providerNameFriendli'
  | 'providerNameChutes'
  | 'providerNameRequesty'
  | 'providerNameVenice'
  | 'providerNameBaseten'
  | 'providerNameLambda'
  | 'providerNameLepton'
  | 'providerNameKluster'
  | 'providerNameNscale'
  | 'providerNameOvhcloud'
  | 'providerNamePublicAi';

const providerNameKeys: Partial<Record<CompatibleProviderId, CompatibleProviderNameKey>> = {
  gemini: 'providerNameGemini',
  groq: 'providerNameGroq',
  cerebras: 'providerNameCerebras',
  openrouter: 'providerNameOpenRouter',
  mistral: 'providerNameMistral',
  deepseek: 'providerNameDeepSeek',
  xai: 'providerNameXai',
  perplexity: 'providerNamePerplexity',
  together: 'providerNameTogether',
  sambanova: 'providerNameSambaNova',
  anthropic: 'providerNameAnthropic',
  fireworks: 'providerNameFireworks',
  deepinfra: 'providerNameDeepInfra',
  dashscope: 'providerNameDashScope',
  cohere: 'providerNameCohere',
  nvidia: 'providerNameNvidia',
  kilo: 'providerNameKilo',
  moonshot: 'providerNameMoonshot',
  zai: 'providerNameZai',
  minimax: 'providerNameMinimax',
  ai21: 'providerNameAi21',
  upstage: 'providerNameUpstage',
  hyperbolic: 'providerNameHyperbolic',
  novita: 'providerNameNovita',
  siliconflow: 'providerNameSiliconFlow',
  featherless: 'providerNameFeatherless',
  nebius: 'providerNameNebius',
  scaleway: 'providerNameScaleway',
  stepfun: 'providerNameStepFun',
  yi: 'providerNameYi',
  baichuan: 'providerNameBaichuan',
  volcengine: 'providerNameVolcengine',
  hunyuan: 'providerNameHunyuan',
  qianfan: 'providerNameQianfan',
  inflection: 'providerNameInflection',
  inception: 'providerNameInception',
  mimo: 'providerNameMimo',
  friendli: 'providerNameFriendli',
  chutes: 'providerNameChutes',
  requesty: 'providerNameRequesty',
  venice: 'providerNameVenice',
  baseten: 'providerNameBaseten',
  lambda: 'providerNameLambda',
  lepton: 'providerNameLepton',
  kluster: 'providerNameKluster',
  nscale: 'providerNameNscale',
  ovhcloud: 'providerNameOvhcloud',
  publicai: 'providerNamePublicAi',
};

const providerDocumentationUrls: Partial<Record<CompatibleProviderId, string>> = {
  gemini: 'https://ai.google.dev/gemini-api/docs',
  groq: 'https://console.groq.com/docs',
  cerebras: 'https://docs.cerebras.ai/',
  openrouter: 'https://openrouter.ai/docs',
  mistral: 'https://docs.mistral.ai/',
  deepseek: 'https://api-docs.deepseek.com/',
  xai: 'https://docs.x.ai/',
  perplexity: 'https://docs.perplexity.ai/',
  together: 'https://docs.together.ai/',
  sambanova: 'https://docs.sambanova.ai/',
  anthropic: 'https://platform.claude.com/docs/en/api/overview',
  fireworks: 'https://docs.fireworks.ai/',
  deepinfra: 'https://docs.deepinfra.com/',
  dashscope: 'https://www.alibabacloud.com/help/en/model-studio/',
  cohere: 'https://docs.cohere.com/',
  nvidia: 'https://docs.api.nvidia.com/nim/docs/api-quickstart',
  kilo: 'https://kilo.ai/docs',
  moonshot: 'https://platform.moonshot.ai/docs',
  zai: 'https://docs.z.ai/',
  minimax: 'https://platform.minimax.io/docs',
  ai21: 'https://docs.ai21.com/',
  upstage: 'https://console.upstage.ai/docs',
  hyperbolic: 'https://docs.hyperbolic.xyz/',
  novita: 'https://novita.ai/docs',
  siliconflow: 'https://docs.siliconflow.com/',
  featherless: 'https://featherless.ai/docs',
  nebius: 'https://docs.nebius.com/studio',
  scaleway: 'https://www.scaleway.com/en/docs/ai-data/generative-apis/',
  stepfun: 'https://platform.stepfun.com/docs',
  yi: 'https://platform.01.ai/docs',
  baichuan: 'https://platform.baichuan-ai.com/docs',
  volcengine: 'https://www.volcengine.com/docs/82379',
  hunyuan: 'https://cloud.tencent.com/document/product/1729',
  qianfan: 'https://cloud.baidu.com/doc/WENXINWORKSHOP/index.html',
  inflection: 'https://developers.inflection.ai/',
  inception: 'https://docs.inceptionlabs.ai/',
  mimo: 'https://platform.xiaomimimo.com/',
  friendli: 'https://friendli.ai/docs',
  chutes: 'https://chutes.ai/docs',
  requesty: 'https://docs.requesty.ai/',
  venice: 'https://docs.venice.ai/',
  baseten: 'https://docs.baseten.co/',
  lambda: 'https://docs.lambda.ai/',
  lepton: 'https://www.lepton.ai/docs',
  kluster: 'https://docs.kluster.ai/',
  nscale: 'https://docs.nscale.com/',
  ovhcloud: 'https://help.ovhcloud.com/csm/en-public-cloud-ai-endpoints-getting-started',
  publicai: 'https://docs.publicai.co/',
};

// Use the icon URL published by each provider site; several providers do not serve an image at /favicon.ico.
const providerFaviconUrls: Partial<Record<CompatibleProviderId, string>> = {
  ai21: 'https://docs.ai21.com/mintlify-assets/_mintlify/favicons/ai21-demo/zc4uXZjknsciGHPW/_generated/favicon/android-chrome-192x192.png',
  anthropic: 'https://platform.claude.com/favicon.ico',
  baichuan: 'https://platform.baichuan-ai.com/favicon.ico',
  baseten: 'https://docs.baseten.co/mintlify-assets/_mintlify/favicons/baseten-preview/a_ByDgeELv6J0NXr/_generated/favicon/android-chrome-192x192.png',
  cerebras: 'https://docs.cerebras.ai/icon.png?icon.0cup_y3abwadv.png',
  chutes: 'https://chutes.ai/favicon.ico',
  cohere: 'https://docs.cohere.com/api/fern-docs/favicon.ico?v=242b6ab08315',
  dashscope: 'https://img.alicdn.com/tfs/TB1ugg7M9zqK1RjSZPxXXc4tVXa-32-32.png',
  deepinfra: 'https://docs.deepinfra.com/mintlify-assets/_mintlify/favicons/deepinfra/uEBMIHUMawrR4czw/_generated/favicon/android-chrome-192x192.png',
  deepseek: 'https://api-docs.deepseek.com/img/favicon.svg',
  featherless: 'https://featherless.ai/favicon.ico',
  fireworks: 'https://docs.fireworks.ai/mintlify-assets/_mintlify/favicons/fireworksai/nfaajxVV_pUTwC36/_generated/favicon/android-chrome-192x192.png',
  friendli: 'https://friendli.ai/docs/mintlify-assets/_mintlify/favicons/friendliai/f_6Xi29vsyX4mJlf/_generated/favicon/android-chrome-192x192.png',
  groq: 'https://console.groq.com/favicon.ico',
  hunyuan: 'https://cloudcache.tencent-cloud.com/qcloud/favicon.ico',
  hyperbolic: 'https://www.hyperbolic.ai/favicon.ico',
  inflection: 'https://framerusercontent.com/images/knot51rbhpixtqNkYngDvKPfaOE.png',
  inception: 'https://docs.inceptionlabs.ai/mintlify-assets/_mintlify/favicons/inception/CR-PGfA5KwciZhoS/_generated/favicon/android-chrome-192x192.png',
  kluster: 'https://raw.githubusercontent.com/kluster-ai/docs/main/images/kluster-logo.png',
  kilo: 'https://kilo.ai/docs/favicon/favicon.ico',
  lambda: 'https://docs.lambda.com/assets/images/lambda/favicon-48x48.svg',
  lepton: 'https://www.nvidia.com/favicon.ico',
  mimo: 'https://platform.xiaomimimo.com/favicon.png',
  minimax: 'https://platform.minimax.io/favicon.ico',
  mistral: 'https://docs.mistral.ai/favicons/favicon.ico',
  moonshot: 'https://platform.kimi.ai/favicon.ico',
  nebius: 'https://docs.tokenfactory.nebius.com/mintlify-assets/_mintlify/favicons/nebius-723e8b65/c_0s9TotwR1LsmMH/_generated/favicon/android-chrome-192x192.png',
  novita: 'https://docs.novita.ai/mintlify-assets/_mintlify/favicons/novitaai/zNDiSNVLSMogrpXl/_generated/favicon/android-chrome-192x192.png',
  nscale: 'https://docs.nscale.com/mintlify-assets/_mintlify/favicons/nscale/KFKbst3Az50PnMGm/_generated/favicon/android-chrome-192x192.png',
  nvidia: 'https://docs.api.nvidia.com/nim/favicon.ico?v=88c98860c938',
  openrouter: 'https://openrouter.ai/docs/mintlify-assets/_mintlify/favicons/openrouter-d02e98a0/EmhfYwFO2tvnkuV-/_generated/favicon/android-chrome-192x192.png',
  ovhcloud: 'https://docs.ovhcloud.com/en/favicon.png',
  perplexity: 'https://docs.perplexity.ai/mintlify-assets/_mintlify/favicons/perplexity/rSPP83rcZL_iw-xo/_generated/favicon/android-chrome-192x192.png',
  publicai: 'https://publicai.co/favicon.ico',
  qianfan: 'https://bce.bdstatic.com/img/newfavicon.ico',
  requesty: 'https://docs.requesty.ai/mintlify-assets/_mintlify/favicons/requesty/bWQG6e3TK2khNMP8/_generated/favicon/android-chrome-192x192.png',
  sambanova: 'https://docs.sambanova.ai/docs/mintlify-assets/_mintlify/favicons/sambanova-systems/tCkoeefWRScUtrir/_generated/favicon/android-chrome-192x192.png',
  scaleway: 'https://www.scaleway.com/en/docs/favicon/favicon.svg',
  siliconflow: 'https://docs.siliconflow.com/mintlify-assets/_mintlify/favicons/siliconflowcom/lsh-9tnLrL1cRHYi/_generated/favicon/android-chrome-192x192.png',
  stepfun: 'https://platform.stepfun.com/docs/mintlify-assets/_mintlify/favicons/stepfun/UaFoV5WshQmKd4Yh/_generated/favicon/android-chrome-192x192.png',
  together: 'https://docs.together.ai/mintlify-assets/_mintlify/favicons/togetherai-52386018/GkjET5UlJhhSwWhA/_generated/favicon/android-chrome-192x192.png',
  upstage: 'https://console.upstage.ai/assets/images/meta/favicon/favicon-256x256.ico',
  venice: 'https://docs.venice.ai/mintlify-assets/_mintlify/favicons/veniceai/bx2bxRtmLEf0wvnq/_generated/favicon/android-chrome-192x192.png',
  volcengine: 'https://portal.volccdn.com/obj/volcfe/misc/favicon.png',
  xai: 'https://docs.x.ai/favicon.ico',
  yi: 'https://platform.01.ai/icon.png?f17992d0c17501d7',
  zai: 'https://z-cdn.chatglm.cn/z-ai/static/logo.svg',
};

type CredentialRoute =
  | { kind: 'chatgpt'; connectionId: string; label: string }
  | { kind: 'compatible'; providerId: CompatibleProviderId; label: string }
  | { kind: 'custom'; provider: CustomProviderSummary; label: string }
  | { kind: 'opencode'; label: string };

type CredentialInputRoute = Exclude<CredentialRoute, { kind: 'custom' }>;
type CredentialVerificationRoute = Exclude<CredentialRoute, { kind: 'custom' }>;

export function ApiKeyProvidersSection({
  chatGptApiKeyFormOpen,
  mode,
  onChanged,
  onChatGptApiKeyFormToggle,
}: ApiKeyProvidersSectionProps) {
  const { t } = useTranslation();
  const chatGptApiAddRoute: CredentialRoute = {
    kind: 'chatgpt',
    connectionId: 'new',
    label: t('providerNameChatGptApi'),
  };
  const [snapshot, setSnapshot] = useState<ProviderCredentialSnapshot | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadFailed, setLoadFailed] = useState(false);
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [openFormKeys, setOpenFormKeys] = useState<Record<string, boolean>>({});
  const [savingKey, setSavingKey] = useState<string | null>(null);
  const [verificationRecords, setVerificationRecords] = useState<Record<string, ProviderVerificationViewState>>({});
  const [verificationResetKeys, setVerificationResetKeys] = useState<Record<string, boolean>>({});
  const [verificationResetFailedKeys, setVerificationResetFailedKeys] = useState<Record<string, boolean>>({});
  const [requestOutcomes, setRequestOutcomes] = useState<Record<string, ProviderRequestOutcomeView>>({});
  const isMountedRef = useRef(true);
  const [removingKey, setRemovingKey] = useState<string | null>(null);
  const [pendingRemoval, setPendingRemoval] = useState<CredentialRoute | null>(null);
  const [editingCustomProvider, setEditingCustomProvider] = useState<CustomProviderSummary | 'new' | null>(null);
  const [savingCustomProvider, setSavingCustomProvider] = useState(false);
  const [customDialogError, setCustomDialogError] = useState<string | null>(null);
  const [feedback, setFeedback] = useState<CredentialFeedback | null>(null);

  const reloadStatus = useCallback(async () => {
    setLoading(true);
    try {
      setSnapshot(await loadProviderCredentialStatus());
      setLoadFailed(false);
    } catch {
      setSnapshot(null);
      setLoadFailed(true);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    isMountedRef.current = true;
    return () => {
      isMountedRef.current = false;
    };
  }, []);

  useEffect(() => {
    void reloadStatus();
  }, [reloadStatus]);

  useEffect(() => {
    if (!snapshot) return;
    let cancelled = false;
    const loadVerificationRecords = async () => {
      const entries = verificationEntries(snapshot);
      const next: Record<string, ProviderVerificationViewState> = {};
      for (const [key, route] of entries) {
        if (verificationResetKeys[key]) {
          next[key] = {
            ...notCheckedVerificationState(),
            saveFailed: verificationResetFailedKeys[key] === true,
          };
          continue;
        }
        try {
          const record = readProviderVerificationRecord(route);
          next[key] = {
            state: record ? providerVerificationStateForOutcome(record.outcome) : 'notChecked',
            checkedAtUnixMs: record?.checkedAtUnixMs ?? null,
            recordReadFailed: false,
            saveFailed: false,
          };
        } catch {
          next[key] = {
            state: 'notChecked',
            checkedAtUnixMs: null,
            recordReadFailed: true,
            saveFailed: false,
          };
        }
      }
      const requestNext: Record<string, ProviderRequestOutcomeView> = {};
      for (const [key, route] of requestOutcomeEntries(snapshot)) {
        try {
          requestNext[key] = { record: readProviderRequestOutcomeRecord(route), readFailed: false, saveFailed: false };
        } catch {
          requestNext[key] = { record: null, readFailed: true, saveFailed: false };
        }
      }
      if (!cancelled && isMountedRef.current) {
        setVerificationRecords(next);
        setRequestOutcomes(requestNext);
      }
    };
    void loadVerificationRecords();
    return () => {
      cancelled = true;
    };
  }, [snapshot, verificationResetFailedKeys, verificationResetKeys]);

  const setDraft = (key: string, value: string) => {
    setDrafts((current) => ({ ...current, [key]: value }));
  };

  const clearDraft = (key: string) => {
    setDrafts((current) => {
      if (!(key in current)) return current;
      const next = { ...current };
      delete next[key];
      return next;
    });
  };

  const toggleCredentialForm = (key: string) => {
    if (mode === 'chatgpt' && key === routeKey(chatGptApiAddRoute) && onChatGptApiKeyFormToggle) {
      if (chatGptApiKeyFormOpen) {
        clearDraft(key);
        setFeedback((current) => (current?.key === key ? null : current));
      }
      onChatGptApiKeyFormToggle();
      return;
    }
    const isClosing = openFormKeys[key] === true;
    if (isClosing) {
      clearDraft(key);
      setFeedback((current) => (current?.key === key ? null : current));
    }
    setOpenFormKeys((current) => ({ ...current, [key]: !current[key] }));
  };

  const closeCredentialForm = (key: string) => {
    if (mode === 'chatgpt' && key === routeKey(chatGptApiAddRoute) && onChatGptApiKeyFormToggle) {
      clearDraft(key);
      setFeedback((current) => (current?.key === key ? null : current));
      if (chatGptApiKeyFormOpen) onChatGptApiKeyFormToggle();
      return;
    }
    clearDraft(key);
    setOpenFormKeys((current) => {
      if (!current[key]) return current;
      return { ...current, [key]: false };
    });
  };

  const resetVerification = (key: string, route: CredentialVerificationRoute) => {
    setVerificationResetKeys((current) => ({ ...current, [key]: true }));
    setVerificationRecords((current) => ({ ...current, [key]: notCheckedVerificationState() }));
    setVerificationResetFailedKeys((current) => {
      if (!current[key]) return current;
      const next = { ...current };
      delete next[key];
      return next;
    });
    try {
      clearProviderVerificationRecord(providerVerificationRouteForCredential(route));
    } catch {
      setVerificationResetFailedKeys((current) => ({ ...current, [key]: true }));
      setVerificationRecords((current) => ({
        ...current,
        [key]: { ...notCheckedVerificationState(), saveFailed: true },
      }));
    }
  };

  const clearRequestOutcome = (key: string, route: ProviderRequestRoute) => {
    let saveFailed = false;
    try {
      clearProviderRequestOutcomeRecord(route);
    } catch {
      saveFailed = true;
    }
    setRequestOutcomes((current) => ({ ...current, [key]: { record: null, readFailed: false, saveFailed } }));
  };

  const saveCredential = async (route: CredentialInputRoute, apiKey: string, feedbackKey = routeKey(route)) => {
    const key = feedbackKey;
    if (savingKey !== null || apiKey.trim().length === 0) return;
    const invalidFormatKey = route.kind === 'chatgpt' ? 'openAiApiKeyInvalidFormat' : 'apiKeyInvalidFormat';
    if (!isValidApiKey(route.kind === 'compatible' ? route.providerId : route.kind, apiKey)) {
      setFeedback({ key, kind: 'error', text: t(invalidFormatKey) });
      return;
    }
    setSavingKey(key);
    setFeedback(null);
    try {
      if (route.kind === 'chatgpt') {
        await saveChatGptApiKey(route.connectionId, apiKey);
      } else if (route.kind === 'compatible') {
        await saveCompatibleProviderApiKey(route.providerId, apiKey);
      } else {
        await saveOpenCodeApiKey(apiKey);
      }
      setDraft(key, '');
      closeCredentialForm(key);
      resetVerification(key, route);
      clearRequestOutcome(key, providerRequestRouteForCredential(route));
      await reloadStatus();
      await onChanged();
      setFeedback({
        key,
        kind: 'success',
        text: t(route.kind === 'chatgpt' ? 'openAiApiKeySaved' : 'apiKeySaved'),
      });
    } catch (error) {
      const code = readErrorCode(error);
      setFeedback({
        key,
        kind: 'error',
        text:
          code === 'provider_credential_duplicate'
            ? t('apiKeyDuplicate')
            : code === 'provider_credential_invalid'
              ? t(invalidFormatKey)
              : t('apiKeyStorageSaveFailed'),
      });
    } finally {
      setSavingKey(null);
    }
  };

  const confirmRemoval = async () => {
    if (!pendingRemoval || removingKey !== null) return;
    const route = pendingRemoval;
    const key = routeKey(route);
    setRemovingKey(key);
    setFeedback(null);
    try {
      if (route.kind === 'chatgpt') {
        await deleteChatGptApiKey(route.connectionId);
      } else if (route.kind === 'compatible') {
        await deleteCompatibleProviderApiKey(route.providerId);
      } else if (route.kind === 'custom') {
        await deleteCustomProvider(route.provider.id);
      } else {
        await deleteOpenCodeApiKey();
      }
      setPendingRemoval(null);
      if (route.kind !== 'custom') resetVerification(key, route);
      clearRequestOutcome(key, providerRequestRouteForCredential(route));
      await reloadStatus();
      await onChanged();
      setFeedback({ key, kind: 'success', text: t('connectionRemoveSucceeded') });
    } catch {
      setFeedback({ key, kind: 'error', text: t('apiKeyStorageRemoveFailed') });
    } finally {
      setRemovingKey(null);
    }
  };

  const saveCustomProvider = async (draft: CustomProviderDraft) => {
    const key = `custom:${draft.id}`;
    setSavingCustomProvider(true);
    setCustomDialogError(null);
    setFeedback(null);
    try {
      await saveCustomProviderRecord(draft);
      setEditingCustomProvider(null);
      clearRequestOutcome(key, { providerId: draft.id, connectionId: draft.id, workspaceId: null });
      await reloadStatus();
      await onChanged();
      setFeedback({ key, kind: 'success', text: t('apiKeySaved') });
    } catch (error) {
      const code = readErrorCode(error);
      setCustomDialogError(
        t(
          code === 'custom_provider_key_required_for_endpoint_change'
            ? 'customProviderKeyRequiredForEndpointChange'
            : code === 'custom_provider_http_ip_or_loopback_required'
              ? 'customProviderHttpIpOrLoopbackRequired'
              : code === 'custom_provider_http_approval_required'
                ? 'customProviderHttpApprovalRequired'
                : code === 'custom_provider_http_approval_denied'
                  ? 'customProviderHttpApprovalDenied'
                  : code === 'provider_credential_invalid'
                    ? 'customProviderInvalid'
                    : 'apiKeyStorageSaveFailed',
        ),
      );
    } finally {
      setSavingCustomProvider(false);
    }
  };

  const isChatGptApiFormOpen =
    mode === 'chatgpt' && onChatGptApiKeyFormToggle
      ? chatGptApiKeyFormOpen === true
      : openFormKeys[routeKey(chatGptApiAddRoute)] === true;
  const openCodeRoute: CredentialRoute = { kind: 'opencode', label: t('openCodeConsole') };
  const openCodeConfigured = snapshot?.openCodeConfigured === true;
  const openCodeKeySuffix = snapshot?.openCodeKeySuffix ?? null;
  const apiKeyLoadingState = loading ? (
    <div className="px-5 py-5">
      <div aria-label={t('providerCredentialStatusLoading')} className="h-0.5 animate-pulse rounded-full bg-accent" role="status" />
    </div>
  ) : null;
  const loadFailureState = loadFailed ? (
    <div
      className={`flex flex-wrap items-center justify-between gap-3 px-5 py-4 ${mode === 'chatgpt' ? '' : 'rounded-card border border-destructive bg-surface'}`}
      role="alert"
    >
      <p className="text-sm text-destructive">{t('apiKeyStorageUnavailable')}</p>
      <Button disabled={loading} onClick={() => void reloadStatus()} size="small">
        {loading ? (
          <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
        ) : (
          <RefreshCw aria-hidden="true" size={16} />
        )}
        {t('retry')}
      </Button>
    </div>
  ) : null;

  const chatGptApiContent =
    snapshot && !loading ? (
      <>
        {snapshot.chatgptApiKeys.length > 0 ? (
          <div className="grid divide-y divide-border">
            {snapshot.chatgptApiKeys.map((entry) => {
              const route: CredentialRoute = {
                kind: 'chatgpt',
                connectionId: entry.id,
                label: t('providerNameChatGptApi'),
              };
              return (
                <SavedChatGptApiKey
                  entry={entry}
                  feedback={feedback?.key === routeKey(route) ? feedback : null}
                  key={entry.id}
                  onRemove={() => setPendingRemoval(route)}
                  removing={removingKey === routeKey(route)}
                  requestOutcome={requestOutcomes[routeKey(route)]}
                  verification={verificationRecords[routeKey(route)]}
                />
              );
            })}
          </div>
        ) : null}
        {isChatGptApiFormOpen ? (
          <div className={`grid gap-2 px-3 py-3 ${snapshot.chatgptApiKeys.length > 0 ? 'border-t border-border' : ''}`}>
            <CredentialInput
              draft={drafts[routeKey(chatGptApiAddRoute)] ?? ''}
              inputLabel={t('openAiApiKeyInputLabel')}
              label={t('providerNameChatGptApi')}
              loading={false}
              onChange={(value) => setDraft(routeKey(chatGptApiAddRoute), value)}
              onSave={() =>
                void saveCredential(
                  { ...chatGptApiAddRoute, connectionId: createCredentialId() },
                  drafts[routeKey(chatGptApiAddRoute)] ?? '',
                  routeKey(chatGptApiAddRoute),
                )
              }
              onCancel={() => toggleCredentialForm(routeKey(chatGptApiAddRoute))}
              saving={savingKey === routeKey(chatGptApiAddRoute)}
              saveLabel={t('apiKeyAddAction')}
            />
          </div>
        ) : null}
        {feedback?.key === routeKey(chatGptApiAddRoute) ? (
          <div className="px-3 pb-3">
            <FeedbackMessage feedback={feedback} />
          </div>
        ) : null}
      </>
    ) : null;

  const modelProviderCards =
    snapshot && !loading ? (
      <div className="grid gap-6">
        <div className="grid min-w-0 gap-2 overflow-hidden rounded-card border border-border bg-raised p-3">
          <CredentialCard
            configured={openCodeConfigured}
            documentationUrl="https://opencode.ai/zen"
            draft={drafts.opencode ?? ''}
            feedback={feedback?.key === routeKey(openCodeRoute) ? feedback : null}
            formOpen={openFormKeys[routeKey(openCodeRoute)] === true}
            inputLabel={t('openCodeApiKey')}
            keySuffix={openCodeKeySuffix}
            label={t('openCodeConsole')}
            loading={false}
            onChange={(value) => setDraft('opencode', value)}
            onRemove={() => setPendingRemoval(openCodeRoute)}
            onSave={() => void saveCredential(openCodeRoute, drafts.opencode ?? '')}
            onToggleForm={() => toggleCredentialForm(routeKey(openCodeRoute))}
            providerId="opencode"
            removing={removingKey === routeKey(openCodeRoute)}
            saving={savingKey === routeKey(openCodeRoute)}
            statusText={
              openCodeConfigured
                ? openCodeKeySuffix
                  ? t('openCodeKeySaved', { suffix: openCodeKeySuffix })
                  : t('providerKeySaved')
                : t('openCodeNoKey')
            }
            requestOutcome={requestOutcomes[routeKey(openCodeRoute)]}
            verification={verificationRecords[routeKey(openCodeRoute)]}
          />

          {snapshot.compatibleProviders.map((credential) => {
            const providerId = credential.providerId;
            const nameKey = providerNameKeys[providerId];
            const route: CredentialRoute = {
              kind: 'compatible',
              providerId,
              label: nameKey ? t(nameKey) : providerId,
            };
            const key = routeKey(route);
            return (
              <CredentialCard
                configured={credential?.isConfigured === true}
                documentationUrl={providerDocumentationUrls[providerId]}
                faviconUrl={providerFaviconUrl(providerId)}
                draft={drafts[key] ?? ''}
                feedback={feedback?.key === key ? feedback : null}
                formOpen={openFormKeys[key] === true}
                inputLabel={t('providerApiKey')}
                key={providerId}
                keySuffix={credential?.keySuffix ?? null}
                label={route.label}
                loading={false}
                onChange={(value) => setDraft(key, value)}
                onRemove={() => setPendingRemoval(route)}
                onSave={() => void saveCredential(route, drafts[key] ?? '')}
                onToggleForm={() => toggleCredentialForm(key)}
                providerId={providerId}
                removing={removingKey === key}
                saving={savingKey === key}
                statusText={
                  credential?.isConfigured
                    ? credential.keySuffix
                      ? t('providerKeySavedSuffix', { suffix: credential.keySuffix })
                      : t('providerKeySaved')
                    : t('providerNoKey')
                }
                requestOutcome={requestOutcomes[key]}
                verification={verificationRecords[key]}
                showGeminiNotice={providerId === 'gemini'}
              />
            );
          })}
        </div>

        <div className="grid min-w-0 gap-3">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <h3 className="font-semibold">{t('customProvidersHeading')}</h3>
            <Button
              disabled={savingCustomProvider}
              onClick={() => {
                setCustomDialogError(null);
                setEditingCustomProvider('new');
              }}
              size="small"
              variant="secondary"
            >
              <Plus aria-hidden="true" size={15} />
              {t('addCustomProvider')}
            </Button>
          </div>
          {snapshot.customProviders.length === 0 ? (
            <p className="text-sm text-secondary-text">{t('customProvidersEmpty')}</p>
          ) : null}
          {snapshot.customProviders.map((provider) => {
            const route: CredentialRoute = { kind: 'custom', provider, label: provider.name };
            const key = routeKey(route);
            return (
              <CustomProviderCard
                feedback={feedback?.key === key ? feedback : null}
                key={provider.id}
                onEdit={() => {
                  setCustomDialogError(null);
                  setEditingCustomProvider(provider);
                }}
                onRemove={() => setPendingRemoval(route)}
                provider={provider}
                removing={removingKey === key}
              />
            );
          })}
        </div>
      </div>
    ) : null;
  const showChatGptApiBody =
    loading || loadFailed || (snapshot !== null && snapshot.chatgptApiKeys.length > 0) || isChatGptApiFormOpen;

  return (
    <>
      {mode === 'chatgpt' ? (
        showChatGptApiBody ? (
          <div className="grid border-t border-border">
            {loadFailureState}
            {loading ? apiKeyLoadingState : chatGptApiContent}
          </div>
        ) : null
      ) : (
        <div className="grid gap-4">
          {loadFailureState}
          {loading ? apiKeyLoadingState : modelProviderCards}
        </div>
      )}

      <Dialog
        onOpenChange={(open) => {
          if (!open && removingKey === null) setPendingRemoval(null);
        }}
        open={pendingRemoval !== null}
        title={t('apiKeyRemoveDialogTitle')}
      >
        <p className="text-sm leading-6 text-secondary-text">
          {t('apiKeyRemoveDialogBody', { provider: pendingRemoval?.label ?? '' })}
        </p>
        <div className="mt-6 flex justify-end gap-2">
          <Button disabled={removingKey !== null} onClick={() => setPendingRemoval(null)}>
            {t('cancel')}
          </Button>
          <Button disabled={removingKey !== null} onClick={() => void confirmRemoval()} variant="destructive">
            {removingKey !== null ? (
              <LoaderCircle aria-hidden="true" className="animate-spin" size={16} />
            ) : (
              <Trash2 aria-hidden="true" size={16} />
            )}
            {t('apiKeyRemoveAction')}
          </Button>
        </div>
      </Dialog>

      {mode === 'modelProviders' ? (
        <CustomProviderDialog
          onOpenChange={(open) => {
            if (!open && !savingCustomProvider) {
              setEditingCustomProvider(null);
              setCustomDialogError(null);
            }
          }}
          onSave={(draft) => void saveCustomProvider(draft)}
          externalError={customDialogError}
          open={editingCustomProvider !== null}
          provider={editingCustomProvider === 'new' ? null : editingCustomProvider}
          saving={savingCustomProvider}
        />
      ) : null}
    </>
  );
}

function providerFaviconUrl(providerId: CompatibleProviderId): string | undefined {
  return providerFaviconUrls[providerId];
}

function routeKey(route: CredentialRoute): string {
  if (route.kind === 'chatgpt')
    return route.connectionId === 'new' ? 'chatgpt_api:add' : `chatgpt_api:${route.connectionId}`;
  if (route.kind === 'compatible') return `compatible:${route.providerId}`;
  if (route.kind === 'custom') return `custom:${route.provider.id}`;
  return 'opencode';
}

function providerVerificationRouteForCredential(route: CredentialVerificationRoute): ProviderVerificationRoute {
  if (route.kind === 'chatgpt') {
    return { providerId: 'chatgpt_api', connectionId: route.connectionId, workspaceId: null };
  }
  if (route.kind === 'compatible') {
    return { providerId: route.providerId, connectionId: route.providerId, workspaceId: null };
  }
  return { providerId: 'opencode', connectionId: 'opencode', workspaceId: null };
}

function providerRequestRouteForCredential(route: CredentialRoute): ProviderRequestRoute {
  if (route.kind === 'custom') {
    return { providerId: route.provider.id, connectionId: route.provider.id, workspaceId: null };
  }
  return providerVerificationRouteForCredential(route);
}

function isValidApiKey(providerId: string, input: string): boolean {
  const value = input.trim();
  if (value.length === 0 || value.length > 4096 || !/^[\x21-\x7E]+$/.test(value)) return false;
  if (providerId === 'chatgpt' || providerId === 'chatgpt_api') return value.startsWith('sk-');
  if (providerId === 'gemini') return value.startsWith('AIza') || value.startsWith('AQ.');
  if (providerId === 'groq') return value.startsWith('gsk_');
  if (providerId === 'openrouter') return value.startsWith('sk-or-v1-');
  return true;
}

function readErrorCode(error: unknown): string | null {
  if (isObject(error) && typeof error.code === 'string') return error.code;
  if (typeof error !== 'string') return null;
  try {
    const parsed: unknown = JSON.parse(error);
    return isObject(parsed) && typeof parsed.code === 'string' ? parsed.code : null;
  } catch {
    return null;
  }
}

function createCredentialId(): string {
  return crypto.randomUUID().replaceAll('-', '');
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
