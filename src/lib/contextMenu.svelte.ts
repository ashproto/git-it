// A single app-wide context menu, opened imperatively from any component.
export type MenuItem = {
  label?: string;
  action?: () => void;
  danger?: boolean;
  disabled?: boolean;
  separator?: boolean;
  submenu?: MenuItem[];
};

function makeMenu() {
  let open = $state(false);
  let x = $state(0);
  let y = $state(0);
  let items = $state<MenuItem[]>([]);

  return {
    get open() {
      return open;
    },
    get x() {
      return x;
    },
    get y() {
      return y;
    },
    get items() {
      return items;
    },
    openAt(clientX: number, clientY: number, menuItems: MenuItem[]) {
      x = clientX;
      y = clientY;
      items = menuItems;
      open = true;
    },
    close() {
      open = false;
    },
  };
}

export const contextMenu = makeMenu();
