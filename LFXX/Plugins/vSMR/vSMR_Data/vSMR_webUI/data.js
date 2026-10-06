/* Bundled offline preview data. The native host replaces this state after the
   ui.ready handshake; keeping a compact real-shape bundle makes index.html
   useful on its own and avoids making browser storage a persistence source. */
window.VSMR_DATA = {
  profiles: [
    {
      name: "LFPG",
      schema_version: 2,
      font: {
        font_name: "Tahoma",
        available_fonts: ["EuroScope", "Consolas", "Segoe UI", "Tahoma", "Arial"],
        weight: "Regular",
        sizes: { one: 10, two: 11, three: 12, four: 13, five: 14 },
		label_font_size: 1
	  },
	  filters: {
		display_modes: {
          active: "Normal",
          items: [
            {
              name: "Normal",
              require_assigned_squawk: false,
              require_clearance: false,
              require_valid_tsat: false,
              require_active_tobt: false,
              max_airborne_altitude_ft: 5500,
              max_airborne_speed_kt: 250,
              statuses: {
                no_status: true, push: true, startup: true, taxi: true, lineup: true,
                departure: true, on_runway: true, airborne: true,
                arrivals: true, no_fpl: true, uncorrelated: true
              }
            },
            {
              name: "Tower",
              require_assigned_squawk: false,
              require_clearance: false,
              require_valid_tsat: false,
              require_active_tobt: false,
              max_airborne_altitude_ft: 5500,
              max_airborne_speed_kt: 250,
              statuses: {
                no_status: false, push: false, startup: false, taxi: true, lineup: true,
                departure: true, on_runway: true, airborne: true,
                arrivals: false, no_fpl: false, uncorrelated: false
              }
            }
          ]
        }
      },
      labels: {
        auto_deconfliction: true,
        definition_detailed_inherits_normal: false,
        rounded_corners: true,
        fit_background_to_text: false,
        squawk_error_color: { r: 255, g: 70, b: 70, a: 255 },
        departure: {
          background_no_status_color: { r: 2, g: 124, b: 255, a: 100 },
          background_on_runway_color: { r: 2, g: 124, b: 255, a: 60 },
          background_no_fpl_color: { r: 70, g: 70, b: 70, a: 180 },
          background_push_color: { r: 2, g: 124, b: 255, a: 100 },
          background_startup_color: { r: 2, g: 124, b: 255, a: 100 },
          background_taxi_color: { r: 2, g: 124, b: 255, a: 100 },
          background_lineup_color: { r: 2, g: 124, b: 255, a: 100 },
          background_departure_color: { r: 80, g: 120, b: 200, a: 100 },
          background_airborne_color: { r: 80, g: 120, b: 200, a: 50 },
          text_on_ground_color: { r: 255, g: 255, b: 255, a: 255 },
          text_airborne_color: { r: 255, g: 255, b: 255, a: 255 },
          definition: [["b:callsign", "sctype"], ["asid", "deprwy"]],
          definition_detailed: [["b:callsign", "sctype"], ["asid", "deprwy"], ["scratchpad"]],
          status_definitions: {
            lnup: {
              definition: [["b:callsign", "sctype"], ["asid", "deprwy"]],
              definition_detailed: [["b:callsign", "sctype"], ["asid", "deprwy"], ["scratchpad"]]
            }
          }
        },
        arrival: {
          background_on_ground_color: { r: 191, g: 87, b: 91, a: 100 },
          background_on_runway_color: { r: 170, g: 50, b: 50, a: 80 },
          background_no_fpl_color: { r: 128, g: 128, b: 128, a: 255 },
          background_airborne_color: { r: 191, g: 87, b: 91, a: 60 },
          text_on_ground_color: { r: 255, g: 255, b: 255, a: 255 },
          text_airborne_color: { r: 255, g: 255, b: 255, a: 255 },
          definition: [["b:callsign", "actype"], ["uk_stand", "scratchpad"]],
          definition_detailed: [["b:callsign", "actype"], ["uk_stand", "scratchpad"], ["origin"]],
          status_definitions: {}
        },
        uncorrelated: {
          background_on_ground_color: { r: 150, g: 22, b: 135, a: 255 },
          background_on_runway_color: { r: 150, g: 22, b: 135, a: 50 },
          text_color: { r: 255, g: 255, b: 255, a: 255 },
          definition: [["systemid"]],
          definition_detailed: [["systemid"]]
        },
        airborne: {
          definition: [["callsign", "sctype", "wake"], ["flightlevel", "tendency", "gs"], ["asid"]],
          definition_detailed: [["b:callsign", "sctype", "wake"], ["flightlevel", "tendency", "gs"], ["asid", "groundstatus"]],
          status_definitions: {}
        }
      },
      rimcas: {
        timer: [60, 45, 30, 15, 0],
        timer_lvp: [120, 90, 60, 30, 0],
        background_color_stage_one: { r: 160, g: 90, b: 30, a: 255 },
        background_color_stage_two: { r: 150, g: 0, b: 0, a: 255 },
        caution_alert_text_color: { r: 0, g: 0, b: 0, a: 255 },
        caution_alert_background_color: { r: 255, g: 255, b: 0, a: 255 },
        warning_alert_text_color: { r: 255, g: 255, b: 255, a: 255 },
        warning_alert_background_color: { r: 255, g: 0, b: 0, a: 255 },
        inactive_alerts: []
      },
      targets: {
        icon_style: "realistic",
        symbol_scale: 1,
        small_icon_boost_resolution_preset: "1080p",
        trail_enabled: true,
        trail_ground_points: 4,
        trail_airborne_points: 8,
        departure: {
          airborne: { r: 255, g: 255, b: 255, a: 255 },
          departure: { r: 214, g: 255, b: 160, a: 255 },
          gate: { r: 80, g: 102, b: 0, a: 255 },
          no_fpl: { r: 128, g: 128, b: 128, a: 255 },
          no_status: { r: 165, g: 165, b: 165, a: 255 },
          push: { r: 160, g: 204, b: 0, a: 255 },
          startup: { r: 120, g: 153, b: 0, a: 255 },
          taxi: { r: 204, g: 255, b: 0, a: 255 },
          lineup: { r: 204, g: 255, b: 0, a: 255 }
        },
        arrival: {
          airborne: { r: 255, g: 255, b: 255, a: 255 },
          gate: { r: 168, g: 109, b: 0, a: 225 },
          on_ground: { r: 255, g: 222, b: 167, a: 225 }
        },
        target_color: { r: 255, g: 242, b: 73, a: 255 }
      },
      approach_insets: {
        extended_lines_length: 15,
        extended_lines_ticks_spacing: 1,
        extended_lines_color: { r: 255, g: 255, b: 255, a: 255 },
        runway_color: { r: 0, g: 0, b: 0, a: 255 }
      },
      rules: { version: 1, items: [] }
    },
    {
      _vsmr: {
        schema_version: 1,
        last_active_profile: "LFPG",
        aviso_presets: {
          airports: {
            LFPG: {
              default: "LFPG Ground",
              items: [{
                name: "LFPG Ground",
                linked_movement: false,
                main: {
                  min_latitude: 49.0052, min_longitude: 2.50333,
                  max_latitude: 49.0303, max_longitude: 2.57476,
                  zoom_level: 9
                }
              }]
            }
          }
        }
      }
    }
  ],
  aviso: {
    type: "FeatureCollection",
    name: "LFPG AVISO preview",
    metadata: { airport: "LFPG", bundled_preview: true, default_color_palette: "dark", color_palettes: ["dark", "light", "real"], background_colors: { dark: "#434A4F", light: "#434A4F", real: "#434A4F" } },
    styles: {
      "surface.taxiway": {
        name: "Taxiways",
        layer: "Surfaces",
        object_type: "Area",
        paint: {
          fill: "#273438", "fill-opacity": 1,
          stroke: "#526065", "stroke-width": 1, "stroke-opacity": 1,
          "palette-overrides": { light: { fill: "#8A807F", stroke: "#595E5B" }, real: { fill: "#8A807F", stroke: "#595E5B" } },
          visible: true
        }
      },
      "line.ground_layout_arrows.preview": {
        name: "Ground layout arrows",
        layer: "Markings",
        object_type: "Line",
        paint: {
          stroke: "#d4bd39", "stroke-width": 1.5,
          "stroke-opacity": 1, visible: true
        }
      },
      "label.taxiways": {
        name: "Taxiway labels",
        layer: "Labels",
        object_type: "Label",
        paint: {
          "text-font": "Tahoma", "text-size": 12,
          "text-color": "#84b7d5", "text-halo-color": "#101719",
          "palette-overrides": { light: { "text-color": "#203033", "text-halo-color": "#DDE5E5" }, real: { "text-color": "#203033", "text-halo-color": "#DDE5E5" } },
          "text-halo-width": 1, zoomLevel: 6, visible: true
        }
      }
    },
    vsmr_groups: [
      {
        group_id: "ground-layout-arrows",
        name: "Ground layout arrows",
        visible: true,
        accent: "#d4bd39",
        preview_metadata: { supplied: true }
      }
    ],
    features: [
      {
        type: "Feature",
        id: "preview.taxiway.area",
        properties: {
          name: "Taxiway preview",
          layer: "Surfaces",
          category: "Taxiways",
          object_type: "Area",
          style_id: "surface.taxiway",
          visible: true
        },
        geometry: {
          type: "Polygon",
          coordinates: [[[2.536, 49.012], [2.542, 49.012], [2.542, 49.014], [2.536, 49.014], [2.536, 49.012]]]
        }
      },
      {
        type: "Feature",
        id: "preview.arrow.line",
        properties: {
          name: "Ground layout arrow",
          layer: "Markings",
          category: "Ground layout arrows",
          object_type: "Line",
          style_id: "line.ground_layout_arrows.preview",
          group_id: "ground-layout-arrows",
          visible: true
        },
        geometry: {
          type: "LineString",
          coordinates: [[2.537, 49.013], [2.541, 49.013]]
        }
      },
      {
        type: "Feature",
        id: "preview.taxiway.label",
        properties: {
          name: "Taxiway N",
          layer: "Labels",
          category: "Taxiway labels",
          object_type: "Label",
          style_id: "label.taxiways",
          "text-field": "N",
          "text-anchor": "center",
          visible: true
        },
        geometry: { type: "Point", coordinates: [2.539, 49.013] }
      }
    ]
  }
};
