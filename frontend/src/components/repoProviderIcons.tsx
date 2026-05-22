import type { SVGProps } from "react";

type IconProps = SVGProps<SVGSVGElement> & { className?: string };

function ProviderSvg({ className, children, ...props }: IconProps) {
  return (
    <svg
      viewBox="0 0 16 16"
      width={16}
      height={16}
      fill="currentColor"
      aria-hidden="true"
      className={className}
      {...props}
    >
      {children}
    </svg>
  );
}

export function GitLabIcon({ className, ...props }: IconProps) {
  return (
    <ProviderSvg className={className} {...props}>
      <path d="M8.01 15.5 6.5 9.5h3l-1.5 6Zm-1.5-6L2.5 1.2a1 1 0 0 1 1.9-.4l1.1 3.4 2.5-7.7a1 1 0 0 1 1.9 0l2.5 7.7 1.1-3.4a1 1 0 0 1 1.9.4L9.49 9.5Z" />
    </ProviderSvg>
  );
}

export function BitbucketIcon({ className, ...props }: IconProps) {
  return (
    <ProviderSvg className={className} {...props}>
      <path d="M1.5 2.5A1 1 0 0 1 2.4 2h11.2a1 1 0 0 1 .9 1.3l-1.8 5.6a1 1 0 0 1-.95.7H6.25l-.55 1.6a.5.5 0 0 0 .47.65h5.58a.5.5 0 0 1 0 1H4.17a1 1 0 0 1-.95-.7L1.5 2.5Zm4.2 5.6h4.6l1.1-3.4H4.6l1.1 3.4Z" />
    </ProviderSvg>
  );
}

export function AzureDevOpsIcon({ className, ...props }: IconProps) {
  return (
    <ProviderSvg className={className} {...props}>
      <path d="M1 3.5 7.2 1.1a1 1 0 0 1 1.3.6l.9 2.6 2.2-1.3a1 1 0 0 1 1.4.4l3.5 6.1a1 1 0 0 1-.9 1.5H8.6L6.8 14a1 1 0 0 1-1.7-.2L1 3.5Zm6.2 1.4 2.4 4.1h4.1L11 5.9 7.2 4.9Zm-1.1-.4L3.6 4.8l2.1 3.6 2.4-3.5Z" />
    </ProviderSvg>
  );
}
