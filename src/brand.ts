// Single source for branding and the "Inspired by" credit (see AGENTS.md, rule 9).
// To remove the credit everywhere, empty `inspiredBy` in brand.json.
import brand from "../brand.json";
import inspiredLogo from "./assets/inspired-by/wispr-flow.svg";

export const BRAND = brand;
export const INSPIRED_BY = brand.inspiredBy?.name ? { ...brand.inspiredBy, logo: inspiredLogo } : null;
