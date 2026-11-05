import { useTranslation } from 'react-i18next';
import { BrandMark } from '@shared/components';

export function DesktopRequiredPage() {
  const { t } = useTranslation();

  return (
    <div className="desktop-required-page">
      <main aria-labelledby="desktop-required-title" className="startup-error-content">
        <section className="startup-error-card">
          <div className="startup-error-card__copy">
            <div className="startup-error-card__heading">
              <h1 className="startup-error-card__title" id="desktop-required-title">
                {t('desktopRequiredTitle')}
              </h1>
            </div>
            <p>{t('desktopRequiredDescription')}</p>
          </div>
        </section>
      </main>
      <footer className="startup-error-footer">
        <span>{t('appTitle')}</span>
        <BrandMark size="footer" />
      </footer>
    </div>
  );
}
