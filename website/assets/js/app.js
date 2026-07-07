/* Git It marketing site — client behavior.
   Progressive enhancement only: the page is fully functional without JS. */
(function () {
  "use strict";

  /* (a) Sticky-nav border: add .is-scrolled once the page moves past 8px. */
  var nav = document.querySelector("nav");
  if (nav) {
    var onScroll = function () {
      nav.classList.toggle("is-scrolled", window.scrollY > 8);
    };
    onScroll();
    window.addEventListener("scroll", onScroll, { passive: true });
  }

  /* (b) Scroll reveal: fade [data-reveal] elements in as they enter view —
     only when the user has not asked to reduce motion. Otherwise reveal
     everything immediately so no content stays hidden. */
  var reveals = document.querySelectorAll("[data-reveal]");
  var allowMotion = window.matchMedia("(prefers-reduced-motion: no-preference)").matches;

  if (!allowMotion || !("IntersectionObserver" in window)) {
    for (var i = 0; i < reveals.length; i++) reveals[i].classList.add("in-view");
    return;
  }

  var io = new IntersectionObserver(function (entries) {
    entries.forEach(function (entry) {
      if (entry.isIntersecting) {
        entry.target.classList.add("in-view");
        io.unobserve(entry.target);
      }
    });
  }, { rootMargin: "0px 0px -10% 0px", threshold: 0.1 });

  reveals.forEach(function (el) { io.observe(el); });
})();
