import openchatLogo from '@assets/brand/logo.svg?no-inline';
import openchatLogoDark from '@assets/brand/logo-dark.svg?no-inline';

type BrandMarkProps = {
  size?: 'footer' | 'titlebar' | 'welcome';
};

export function BrandMark({ size = 'titlebar' }: BrandMarkProps) {
  return (
    <span aria-hidden="true" className={`openchat-brand-mark openchat-brand-mark--${size}`}>
      <img alt="" className="openchat-brand-mark__image openchat-brand-mark__image--light" src={openchatLogo} />
      <img alt="" className="openchat-brand-mark__image openchat-brand-mark__image--dark" src={openchatLogoDark} />
    </span>
  );
}
