export interface PackageMapping {
  mathjax: string[];
  xetex: string[];
  katex: boolean;
}

export const PACKAGE_MAP: Record<string, PackageMapping> = {
  base:       { mathjax: ["base"],       xetex: [],          katex: true  },
  ams:        { mathjax: ["ams"],        xetex: ["amsmath"], katex: true  },
  physics:    { mathjax: ["physics"],    xetex: ["physics"], katex: false },
  boldsymbol: { mathjax: ["boldsymbol"], xetex: ["bm"],      katex: true  },
  textmacros: { mathjax: ["textmacros"], xetex: [],          katex: true  },
  bboldx:     { mathjax: ["bboldx"],     xetex: ["bboldx"],  katex: false },
};
