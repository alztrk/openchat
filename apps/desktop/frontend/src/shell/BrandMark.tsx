import openchatLogo from '@assets/brand/logo.svg?no-inline';

type BrandMarkProps = {
  size?: 'footer' | 'titlebar' | 'welcome';
};

export function BrandMark({ size = 'titlebar' }: BrandMarkProps) {
  return (
    <span aria-hidden="true" className={`openchat-brand-mark openchat-brand-mark--${size}`}>
      <img alt="" className="openchat-brand-mark__image" src={openchatLogo} />
    </span>
  );
}
