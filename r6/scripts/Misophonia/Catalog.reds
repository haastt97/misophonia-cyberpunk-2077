module Misophonia

public struct SoundCategory {
  public let section: CName;
  public let setting: CName;
  public let events: array<CName>;
}

public abstract class Catalog {
  public static func Create() -> array<SoundCategory> {
    let result: array<SoundCategory>;
    let entry: SoundCategory;
    entry.section = n"allFood";
    entry.setting = n"food";
    entry.events = [n"cmn_generic_male_eat_chrunchy", n"cmn_generic_male_eat_generic", n"cmn_generic_male_eat_wet", n"cmn_generic_male_fat_sit_eat_burger_01", n"cmn_generic_male_fat_sit_eat_burger_02", n"cmn_generic_male_fat_stand_eat_burger_01", n"cmn_generic_male_fat_stand_eat_burger_02", n"generic_work_eat_long", n"generic_work_eat_short", n"q112_sc_01_2_takemura_eats_yakitori", n"q112_sc_01_2_v_eats_yakitori", n"sq021_sc_05_joss_stops_eat", n"ui_loot_eat", n"ui_loot_eat_ui"];
    ArrayPush(result, entry);
    entry.section = n"allFood";
    entry.setting = n"drinking";
    entry.events = [n"cmn_generic_work_bottle_drink", n"generic_work_drink", n"mq008_drink_shots", n"ono_v_inhale_post_drink", n"q000_corpo_sc_01_v_fem_drinks", n"q000_corpo_sc_01_v_male_drinks", n"q000_corpo_sc_03b_v_drinks", n"q000_corpo_sc_04_jackie_drinks", n"q000_corpo_sc_04_jackie_drinks_alone", n"q000_kid_sc_01b_v_drinks", n"q001_sc_01_v_cant_drink", n"q001_sc_01_v_drinks", n"q001_sc_01_v_starts_drinking_whiskey", n"q001_sc_01_v_starts_drinking_whiskey_end", n"q001_sc_01_v_starts_drinking_whiskey_mid", n"q004_sc_05_evelyn_drinks", n"q004_sc_05_evelyn_drinks_rev", n"q103_sc_15_v_drinks", n"q108_sc_05_thug_drinks", n"q108_sc_10_johnny_drinks", n"q108_sc_10_johnny_stands_and_drink", n"q108_sc_13_johnny_drinks", n"q108_sc_13_johnny_drinks_beer_01", n"q108_sc_13_johnny_drinks_beer_02", n"q110_sc_02_granny_drinks", n"q114_sc_03b_v_grabs_beer_and_drink", n"q114_sc_04b_v_drinks", n"q114_sc_04b_veterans_drink_01", n"q114_sc_04b_veterans_drink_02", n"q203_sc_01_v_drinks_coffee", n"q203_sc_05_v_drinks", n"q303_sc_04_hands_drinks", n"q303_sc_04_v_drinks", n"q306_sc_07_bottle_drink", n"sq004_sc_08_panam_drinks", n"sq017_sc_06_v_takes_and_drinks_coffee", n"sq027_sc_04a_v_toast_drink", n"sq029_sc_05_v_drinks_coffee", n"sq030_sc_11_v_drinks_coffee", n"sq031_sc_01_drink_competition", n"sq031_sc_01_johnny_drink_competition_01", n"sq031_sc_01_johnny_drink_competition_02", n"sq031_sc_01_johnny_drink_competition_03", n"sq031_sc_01_johnny_drink_competition_transition", n"sq031_sc_01a_johnny_drinks_01", n"sq031_sc_01a_johnny_drinks_02", n"sq031_sc_01a_johnny_drinks_03", n"sq031_sc_01a_johnny_drinks_transition", n"sq031_sc_01b_johnny_drinks", n"ui_loot_drink"];
    ArrayPush(result, entry);
    entry.section = n"allFood";
    entry.setting = n"food_misc";
    entry.events = [n"cmn_generic_work_food_takeout", n"ph_food_box_soft", n"ph_food_soft", n"ph_plastic_food_package_soft"];
    ArrayPush(result, entry);
    entry.section = n"allFood";
    entry.setting = n"slurping";
    entry.events = [n"cmn_generic_male_eat_slurpy", n"sq017_sc_06_v_slurps_coffee", n"sq030_sc_11_v_slurps_coffee"];
    ArrayPush(result, entry);
    entry.section = n"allFood";
    entry.setting = n"swallowing";
    entry.events = [n"cmn_generic_female_drink_swallow", n"g_sc_npc_work_swallow", n"g_sc_v_work_swallow", n"generic_work_swallow", n"q005_sc_01a_jackie_swallows", n"q302_04_squot_myers_swallow", n"q304_sc_04e_alex_drink_swallow", n"sq017_sc_06_kerry_swallows_coffee"];
    ArrayPush(result, entry);
    entry.section = n"allFood";
    entry.setting = n"cutlery";
    entry.events = [n"ph_ceramic_plate_solid", n"ph_metal_cutlery_solid", n"sq021_sc_05_cutlery", n"sq021_sc_09_anthony_fork_plate", n"sq021_sc_09_rev_anthony_fork_plate"];
    ArrayPush(result, entry);
    entry.section = n"allBody";
    entry.setting = n"coughing";
    entry.events = [n"amb_bl_ext_roadhouse_cough", n"cmn_generic_female_cough", n"cmn_generic_kid_cough", n"cmn_generic_male_cough", n"cmn_generic_male_cough_series", n"g_sc_v_sickness_cough_blood", n"g_sc_v_sickness_cough_hard", n"g_sc_v_sickness_cough_light", n"q110_sc_02_granny_coughs", n"q114_sc_02_v_vo_cough", n"q115_sc_00b_vo_v_cough", n"q203_sc_01_v_female_cough", n"q203_sc_01_v_male_cough", n"sq004_sc_09_saul_coughing", n"sq029_sc_06a_cough"];
    ArrayPush(result, entry);
    entry.section = n"allBody";
    entry.setting = n"throat_clearing";
    entry.events = [n"cmn_generic_kid_clear_throat"];
    ArrayPush(result, entry);
    entry.section = n"allBody";
    entry.setting = n"sniffing";
    entry.events = [n"cmn_generic_female_drugged_sniff_short", n"cmn_generic_female_drugged_sniff_short_rare", n"cmn_generic_male_sniff", n"cmn_generic_work_sniff_few_short", n"cmn_generic_work_sniff_long", n"cmn_generic_work_sniff_short", n"q303_sc_04_hands_sniff"];
    ArrayPush(result, entry);
    entry.section = n"allBody";
    entry.setting = n"breathing";
    entry.events = [n"cmn_generic_female_breathing_shivering", n"cmn_generic_female_breathing_stressed", n"cmn_generic_female_drugged_breathing_sick", n"cmn_generic_female_exhale", n"cmn_generic_female_inhale", n"cmn_generic_female_sleep_exhale", n"cmn_generic_female_sleep_inhale", n"cmn_generic_kid_breath_playful", n"cmn_generic_male_breathing_stressed", n"cmn_generic_male_drugged_breathing_sick", n"cmn_generic_male_exhale", n"cmn_generic_male_inhale", n"cmn_generic_male_meditation_exhale", n"cmn_generic_male_meditation_inhale", n"cmn_generic_man_cold_exhale", n"cmn_generic_woman_cold_exhale"];
    ArrayPush(result, entry);
    entry.section = n"allBody";
    entry.setting = n"snoring";
    entry.events = [n"cmn_generic_male_sleep_snore"];
    ArrayPush(result, entry);
    entry.section = n"allBody";
    entry.setting = n"yawning";
    entry.events = [n"cmn_generic_female_yawn", n"cmn_generic_male_yawn"];
    ArrayPush(result, entry);
    entry.section = n"allBody";
    entry.setting = n"burping";
    entry.events = [n"cmn_generic_male_average_burp", n"cmn_generic_male_fat_burp", n"q003_sc_03_deal_maelstrom_burp"];
    ArrayPush(result, entry);
    entry.section = n"allBody";
    entry.setting = n"spitting";
    entry.events = [n"cmn_generic_female_spit_fast", n"cmn_generic_male_spit_fast", n"q000_nomad_sc_01_sherif_spits", n"q110_sc_02_granny_spits", n"q110_sc_02_granny_spits_long"];
    ArrayPush(result, entry);
    entry.section = n"allBody";
    entry.setting = n"kissing";
    entry.events = [n"q108_sc_04_kerry_kisses", n"sq004_sc_09_panam_kisses_v", n"sq029_sc_04a_v_kisses_river", n"sq30_sc_09_judy_kisses_v"];
    ArrayPush(result, entry);
    entry.section = n"allBody";
    entry.setting = n"hiccups";
    entry.events = [n"cmn_generic_female_drunk_hiccup", n"cmn_generic_male_drunk_hiccup", n"cmn_generic_male_fat_hiccup"];
    ArrayPush(result, entry);
    entry.section = n"allBody";
    entry.setting = n"gagging";
    entry.events = [n"cmn_generic_female_gag_reflex", n"cmn_generic_male_gag_reflex"];
    ArrayPush(result, entry);
    entry.section = n"allBody";
    entry.setting = n"retching";
    entry.events = [n"amb_q307_cabride_comm_tunnel_vomiting", n"cmn_generic_female_vomit", n"cmn_generic_male_drugged_pre_vomit", n"cmn_generic_male_vomit", n"q000_corpo_sc_01_v_fem_vomits", n"q000_corpo_sc_01_v_male_vomits", n"sq031_sc_01_v_vo_vomit", n"sq031_sc_01d_v_vomit"];
    ArrayPush(result, entry);
    entry.section = n"allBody";
    entry.setting = n"joints";
    entry.events = [n"cmn_generic_work_neck_crack", n"q_ma_street_violence_02_crack_knuckles"];
    ArrayPush(result, entry);
    entry.section = n"allBody";
    entry.setting = n"choking";
    entry.events = [n"ono_animals_f_choking_set", n"ono_animals_f_choking_set_01", n"ono_animals_f_choking_set_02", n"ono_animals_f_choking_set_03", n"ono_animals_f_choking_set_04", n"ono_animals_m_choking_set", n"ono_animals_m_choking_set_01", n"ono_animals_m_choking_set_02", n"ono_animals_m_choking_set_03", n"ono_animals_m_choking_set_04", n"ono_animals_m_choking_set_05", n"ono_finisher_f_choking_set_03", n"ono_finisher_m_choking_set_04", n"ono_freak_f_choking_set", n"ono_freak_f_choking_set_01", n"ono_freak_f_choking_set_02", n"ono_freak_f_choking_set_03", n"ono_freak_f_choking_set_04", n"ono_freak_m_choking_set", n"ono_freak_m_choking_set_01", n"ono_freak_m_choking_set_02", n"ono_freak_m_choking_set_03", n"ono_freak_m_choking_set_04", n"ono_freak_m_choking_set_05", n"ono_generic_f_choking_set_01", n"ono_generic_f_choking_set_02", n"ono_generic_f_choking_set_03", n"ono_generic_f_choking_set_04", n"ono_generic_m_choking_set_01", n"ono_generic_m_choking_set_02", n"ono_generic_m_choking_set_03", n"ono_generic_m_choking_set_04", n"ono_generic_m_choking_set_05", n"ono_helmet_f_choking_set", n"ono_helmet_f_choking_set_01", n"ono_helmet_f_choking_set_02", n"ono_helmet_f_choking_set_03", n"ono_helmet_f_choking_set_04", n"ono_helmet_m_choking_set", n"ono_helmet_m_choking_set_01", n"ono_helmet_m_choking_set_02", n"ono_helmet_m_choking_set_03", n"ono_helmet_m_choking_set_04", n"ono_helmet_m_choking_set_05", n"ono_maelstrom_f_choking_set_01", n"ono_maelstrom_f_choking_set_02", n"ono_maelstrom_f_choking_set_03", n"ono_maelstrom_f_choking_set_04", n"ono_maelstrom_m_choking_set_01", n"ono_maelstrom_m_choking_set_02", n"ono_maelstrom_m_choking_set_03", n"ono_maelstrom_m_choking_set_04", n"ono_maelstrom_m_choking_set_05", n"ono_v_choking", n"q001_sc_00c_flatlined_choke", n"q005_sc_07_yorinobu_chokes_01", n"q005_sc_07_yorinobu_chokes_02"];
    ArrayPush(result, entry);
    entry.section = n"allRepetitive";
    entry.setting = n"tapping";
    entry.events = [n"cmn_generic_work_arm_tapping", n"cmn_generic_work_hand_tap_tabletop", n"q303_sc_07_reed_finger_tap", n"q303_sc_07_reed_tap_table_hard_01", n"q303_sc_07_reed_tap_table_hard_02", n"q303_sc_07_reed_tap_table_light", n"q304_sc_04d_reed_tap_table", n"q304_sc_04d_reed_tap_table_light_01", n"q304_sc_04d_reed_tap_table_light_02", n"q304_sc_04e_v_finger_tap", n"q_ow_cz_con_gunsmith_01_hand_tap_tabletop"];
    ArrayPush(result, entry);
    entry.section = n"allRepetitive";
    entry.setting = n"typing";
    entry.events = [n"cmn_generic_work_keyboard_tap", n"q101_sc_06c_cyberdeck_keyboard_01", n"q101_sc_06c_cyberdeck_keyboard_02", n"q101_sc_06c_doc_keyboard", n"q105_sc_07_blackmarket_bd_scav_keyboard", n"q105_sc_07_blackmarket_bd_scav_keyboard_rev", n"q105_sc_07_keyboard_rev", n"q110_sc_08a_agent_keyboard_tap_01", n"q110_sc_08a_agent_keyboard_tap_02", n"q304_sc_07b_songbird_tap_keyboard", n"q304_sc_07b_songbird_tap_keyboard_02", n"q304_sc_07b_songbird_tap_keyboard_03", n"q305_sc_04_reed_keyboard_tap", n"q_sts_ep1_08_agent_keyboard"];
    ArrayPush(result, entry);
    entry.section = n"allRepetitive";
    entry.setting = n"phone_taps";
    entry.events = [n"cmn_generic_work_phone_tap_long", n"cmn_generic_work_phone_tap_single", n"q304_sc_07a_guard_tap_tablet", n"q304_sc_07a_kurtz_tap_tablet"];
    ArrayPush(result, entry);
    entry.section = n"allRepetitive";
    entry.setting = n"rustling";
    entry.events = [
      n"lcm_fs_additional_paper_hand_grab",
      n"lcm_fs_additional_paper_slide",
      n"lcm_fs_additional_plastic_hand_grab",
      n"lcm_fs_additional_plastic_slide",
      n"lcm_fs_npc_additional_paper_hand_grab",
      n"lcm_fs_npc_additional_paper_slide",
      n"lcm_fs_npc_additional_plastic_hand_grab",
      n"ph_paper_book_soft",
      n"ph_paper_soft",
      n"ph_newspaper_pile_soft",
      n"ph_newspaper_stand",
      n"q204_sc_01_v_takes_book",
      n"q301_sc_00a_v_put_down_book",
      n"sq018_02_v_takes_book",
      n"sq018_sc_03_v_puts_book",
      n"q000_kid_sc_01b_kirk_puts_paper",
      n"q000_kid_sc_01b_kirk_takes_paper",
      n"q000_kid_sc_01b_v_grabs_paper",
      n"q000_kid_sc_01b_v_puts_paper",
      n"q000_nomad_sc_05_officer_checks_papers",
      n"q000_nomad_sc_05_papers_01",
      n"q000_nomad_sc_05_papers_02",
      n"q000_nomad_sc_05_papers_03",
      n"q000_nomad_sc_05_papers_04",
      n"q000_nomad_sc_05_v_hide_papers",
      n"q000_nomad_sc_05_v_puts_papers",
      n"q000_nomad_sc_05_v_takes_papers",
      n"q000_nomad_sc_05_v_takes_papers_01",
      n"q203_sc_05_blue_eyes_gives_leaflet",
      n"q203_sc_06_v_leaflet"
    ];
    ArrayPush(result, entry);
    entry.section = n"allRepetitive";
    entry.setting = n"scuffs";
    entry.events = [n"lcm_fs_bare_feet_concrete_shuffle", n"lcm_fs_bare_feet_linoleum_shuffle", n"lcm_fs_bare_feet_wood_hollow_shuffle", n"lcm_fs_heavy_boots_concrete_shuffle", n"lcm_fs_heavy_boots_linoleum_shuffle", n"lcm_fs_heavy_boots_wood_hollow_shuffle", n"lcm_fs_heels_concrete_shuffle", n"lcm_fs_heels_linoleum_shuffle", n"lcm_fs_heels_wood_hollow_shuffle", n"lcm_fs_npc_bare_feet_concrete_shuffle", n"lcm_fs_npc_bare_feet_linoleum_shuffle", n"lcm_fs_npc_bare_feet_wood_hollow_shuffle", n"lcm_fs_npc_heavy_boots_concrete_shuffle", n"lcm_fs_npc_heavy_boots_linoleum_shuffle", n"lcm_fs_npc_heavy_boots_wood_hollow_shuffle", n"lcm_fs_npc_heels_concrete_shuffle", n"lcm_fs_npc_heels_linoleum_shuffle", n"lcm_fs_npc_heels_wood_hollow_shuffle", n"lcm_fs_npc_metal_feet_concrete_shuffle", n"lcm_fs_npc_metal_feet_linoleum_shuffle", n"lcm_fs_npc_metal_feet_wood_hollow_shuffle", n"lcm_fs_npc_sneakers_concrete_shuffle", n"lcm_fs_npc_sneakers_linoleum_shuffle", n"lcm_fs_npc_sneakers_wood_hollow_shuffle", n"lcm_fs_npc_stilettos_concrete_shuffle", n"lcm_fs_npc_stilettos_linoleum_shuffle", n"lcm_fs_npc_stilettos_wood_hollow_shuffle", n"lcm_fs_sneakers_concrete_shuffle", n"lcm_fs_sneakers_linoleum_shuffle", n"lcm_fs_sneakers_wood_hollow_shuffle", n"lcm_fs_stilettos_concrete_shuffle", n"lcm_fs_stilettos_linoleum_shuffle", n"lcm_fs_stilettos_wood_hollow_shuffle"];
    ArrayPush(result, entry);
    entry.section = n"allEnvironmental";
    entry.setting = n"clock";
    entry.events = [n"q201_sc_06_clock_ticking_loop"];
    ArrayPush(result, entry);
    entry.section = n"allEnvironmental";
    entry.setting = n"drips";
    entry.events = [n"amb_g_fx_water_drip_medium_01", n"amb_g_fx_water_drip_medium_02_faar", n"amb_g_fx_water_drip_medium_02_far", n"amb_g_fx_water_drip_medium_03_far", n"amb_g_fx_water_drip_medium_04_close", n"amb_g_fx_water_drip_medium_04_closer", n"amb_g_fx_water_drip_small_01", n"amb_g_fx_water_drip_small_rnd_01", n"amb_g_fx_water_drip_small_rnd_02", n"amb_g_fx_water_drip_tap_sink_01_light", n"amb_g_fx_water_drip_tap_sink_01_strong", n"amb_bl_int_roadhouse_room_shower_water_dripping", n"q003_amb_meat_factory_water_drips"];
    ArrayPush(result, entry);
    entry.section = n"allEnvironmental";
    entry.setting = n"electric";
    entry.events = [n"amb_bl_int_farm_light_buzz", n"amb_g_fx_electr_buzz_area_01", n"amb_g_fx_electr_buzz_area_02", n"amb_int_EM_elem_electronic_circuit_buzz_loop_01", n"amb_int_EM_elem_electronic_circuit_buzz_loop_01_low", n"dev_terminal_hum", n"q101_sc_07_surgery_lamp_hum_loop"];
    ArrayPush(result, entry);
    entry.section = n"allEnvironmental";
    entry.setting = n"cats";
    entry.events = [n"amb_g_city_el_animals_01_cat_meow_01", n"amb_g_city_el_animals_01_cat_purr_01", n"anm_cat_growl", n"anm_cat_hiss", n"anm_cat_purring_start", n"anm_cat_scream"];
    ArrayPush(result, entry);
    entry.section = n"allEnvironmental";
    entry.setting = n"birds";
    entry.events = [n"amb_q113_arasaka_tower_jungle_bird_rnd_01", n"amb_q113_arasaka_tower_yorinobu_corridor_bird", n"amb_q307_hospital_outside_bird_01", n"amb_q307_hospital_outside_bird_02", n"amb_q_sts_ep1_08_bird_calm"];
    ArrayPush(result, entry);
    entry.section = n"allVoice";
    entry.setting = n"whistles";
    entry.events = [n"radio_q201_v_whistle", n"sq029_sc_06a_whistles"];
    ArrayPush(result, entry);
    entry.section = n"allVoice";
    entry.setting = n"humming";
    entry.events = [n"cmn_generic_female_drugged_humming", n"q_ow_cbj_ep1_06_humming"];
    ArrayPush(result, entry);
    entry.section = n"allVoice";
    entry.setting = n"whispering";
    entry.events = [n"sq029_sc_06a_whispers"];
    ArrayPush(result, entry);
    entry.section = n"allVoice";
    entry.setting = n"crying";
    entry.events = [n"q003_amb_steinbeck_indoor_crying", n"q_ma_robbery_02_victims_crying_01_loop", n"sq023_sc_10_breath_crying_long", n"vo_sq023_gloria_crying"];
    ArrayPush(result, entry);
    entry.section = n"allGraphic";
    entry.setting = n"wet_gore";
    entry.events = [n"w_melee_gore_blood_medium", n"w_melee_gore_blood_short", n"w_melee_gore_blood_splat_small", n"w_melee_gore_blood_squish", n"w_melee_npc_gore_blood_medium", n"w_melee_npc_gore_blood_short", n"w_melee_npc_gore_blood_splat_small", n"w_melee_npc_gore_blood_squish", n"w_headshot_explosion_gore", n"w_melee_body_disposal_bodyfall_splatter", n"w_melee_body_disposal_takedown_gore_additional", n"w_melee_body_disposal_takedown_hit_gore", n"w_melee_finisher_takedown_gore", n"q105_sc_07_gore_rev", n"q_sts_ep1_03_rinder_shot_gore"];
    ArrayPush(result, entry);
    entry.section = n"allGraphic";
    entry.setting = n"bone_breaks";
    entry.events = [n"w_melee_gore_bones_break", n"w_melee_gore_bones_heavy", n"w_melee_gore_bones_light", n"w_melee_npc_gore_bones_break", n"w_melee_npc_gore_bones_heavy", n"w_melee_npc_gore_bones_light", n"w_melee_grapple_animals_neckbrake"];
    ArrayPush(result, entry);
    entry.section = n"allGraphic";
    entry.setting = n"flesh_impacts";
    entry.events = [n"npc_w_melee_borg_machete_imp_flesh", n"w_bul_hit_flesh", n"w_bul_hit_flesh_heavy", n"w_bul_hit_flesh_railgun", n"w_bul_hit_flesh_shotgun", n"w_bul_impact_npc_flesh_critical", n"w_bul_impact_npc_flesh_hi", n"w_bul_impact_npc_flesh_low", n"w_bul_impact_npc_flesh_med", n"w_bul_impact_npc_flesh_metal", n"w_bul_impact_player_flesh", n"w_bul_npc_hit_flesh", n"w_bul_npc_hit_flesh_auto", n"w_bul_npc_hit_flesh_heavy", n"w_bul_npc_hit_flesh_railgun", n"w_bul_npc_hit_flesh_shotgun", n"w_cyb_mantis_impact_flesh", n"w_cyb_mantis_impact_flesh_heavy", n"w_cyb_npc_mantis_impact_flesh", n"w_cyb_npc_mantis_impact_flesh_heavy", n"w_melee_borg_machete_imp_flesh", n"w_melee_katana_impact_flesh"];
    ArrayPush(result, entry);
    return result;
  }

  public static func ScreenGain(percent: Int32) -> Float {
    // Convert linear amplitude to mixer gain. AudioXL treats -72 dB as silence.
    let gain: array<Float> = [
      -72.000000, -40.000000, -33.979400, -30.457575, -27.958800, -26.020600, -24.436975, -23.098039,
      -21.938200, -20.915150, -20.000000, -19.172146, -18.416375, -17.721133, -17.077439, -16.478175,
      -15.917600, -15.391022, -14.894550, -14.424928, -13.979400, -13.555614, -13.151546, -12.765443,
      -12.395775, -12.041200, -11.700533, -11.372725, -11.056839, -10.752040, -10.457575, -10.172766,
      -9.897000, -9.629721, -9.370422, -9.118639, -8.873950, -8.635966, -8.404328, -8.178708,
      -7.958800, -7.744323, -7.535014, -7.330631, -7.130946, -6.935750, -6.744843, -6.558043,
      -6.375175, -6.196078, -6.020600, -5.848596, -5.679933, -5.514483, -5.352125, -5.192746,
      -5.036239, -4.882503, -4.731440, -4.582960, -4.436975, -4.293403, -4.152166, -4.013189,
      -3.876401, -3.741733, -3.609121, -3.478504, -3.349822, -3.223018, -3.098039, -2.974833,
      -2.853350, -2.733543, -2.615366, -2.498775, -2.383728, -2.270185, -2.158108, -2.047458,
      -1.938200, -1.830300, -1.723723, -1.618438, -1.514414, -1.411621, -1.310031, -1.209615,
      -1.110347, -1.012200, -0.915150, -0.819172, -0.724243, -0.630341, -0.537443, -0.445528,
      -0.354575, -0.264565, -0.175478, -0.087296, 0.000000
    ];
    return gain[Clamp(percent, 0, 100)];
  }
}
