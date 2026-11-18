/** An original figure exercising paths, a gradient, a clip and a nested data-URI SVG. */
export const SAMPLE_BACKGROUND = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 600 500">
  <defs>
    <linearGradient id="sky" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" stop-color="#DCEBFA"/>
      <stop offset="1" stop-color="#FFFFFF"/>
    </linearGradient>
    <clipPath id="window"><circle cx="470" cy="110" r="70"/></clipPath>
  </defs>
  <rect width="600" height="500" fill="url(#sky)"/>
  <g clip-path="url(#window)">
    <rect x="390" y="30" width="160" height="160" fill="#7FB3E6"/>
    <rect x="390" y="120" width="160" height="70" fill="#4C8C4A"/>
  </g>
  <circle cx="470" cy="110" r="70" fill="none" stroke="#3A4A5C" stroke-width="6"/>
  <g fill="none" stroke="#3A4A5C" stroke-width="10" stroke-linecap="round" stroke-linejoin="round">
    <circle cx="200" cy="120" r="50" fill="#F2D6C2"/>
    <path d="M200 170 L200 330"/>
    <path d="M120 230 L200 200 L280 230"/>
    <path d="M200 330 L150 450"/>
    <path d="M200 330 L250 450"/>
  </g>
  <image x="60" y="360" width="90" height="90" href="data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCAxMDAgMTAwIj48cGF0aCBkPSJNNTAgNSBMNjEgMzggTDk2IDM4IEw2OCA1OSBMNzkgOTMgTDUwIDcyIEwyMSA5MyBMMzIgNTkgTDQgMzggTDM5IDM4IFoiIGZpbGw9IiNGNUI4MDAiLz48L3N2Zz4="/>
</svg>`;
