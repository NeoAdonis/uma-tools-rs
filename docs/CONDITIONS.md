# Conditions Reference

*(The following information has been taken from GameTora's [Skill Condition Viewer](https://gametora.com/umamusume/skill-condition-viewer) and tweaked.)*

Each section below describes a variable used in skill conditions, with an example comparison and its meaning. See the `condition::parser` module for the condition syntax.

A skill can have several alternatives (condition sets, called "triggers" in GameTora). They're checked in order, and a skill activates through a single alternative per race, except for continuations: alternatives using `is_activate_other_skill_detail`.

## accumulatetime

The number of seconds since the race has started.

*Example:* `accumulatetime>=5`

*Meaning:* The race has been ongoing for at least 5 seconds.

## activate_count_all

The number of skills you have activated in the race.

*Example:* `activate_count_all>=7`

*Meaning:* You have activated at least 7 skills so far.

## activate_count_all_team

The characters on your team have cumulatively activated the specified number of skills.

*Example:* `activate_count_all_team>=10`

*Meaning:* The characters on your team have cumulatively activated 10 skills or more.

## activate_count_end_after

The number of skills you have activated in the late-race or later.

*Example:* `activate_count_end_after>=3`

*Meaning:* You've activated at least 3 skills during the late-race or the last leg.

## activate_count_heal

The number of recovery (healing) skills you have activated during the race.

*Example:* `activate_count_heal>=1`

*Meaning:* You have activated at least one recovery skill during the race.

## activate_count_later_half

You've activated the specified amount of skills in the second half of the race.

*Example:* `activate_count_later_half>=2`

*Meaning:* You've activated at least 2 skills in the second half of the race.

## activate_count_middle

The number of skills you have activated in the Mid-race.

*Example:* `activate_count_middle>=3`

*Meaning:* You've activated at least 3 skills during the Mid-race.

## activate_count_param_skill

The number of green (passive, stat-increasing) skills you have activated in the race.

*Example:* `activate_count_param_skill>=7`

*Meaning:* You've activated at least 7 green skills.

## activate_count_start

The number of skills you have activated in the Early-race.

*Example:* `activate_count_start>=3`

*Meaning:* You've activated at least 3 skills during the Early-race.

## all_corner_random

Picks a random point during any corner.

*Note:* More precisely, this condition randomly picks four points (each time rolls a random corner, and then a random point on that corner). That means that if the skill in question has a short cooldown, there are multiple points where it can activate this way.

*Example:* `all_corner_random==1`

*Meaning:* A random point on a random corner is selected.

## always

Always activates.

*Example:* `always==1`

*Meaning:* This skill has no particular conditions.

## base_guts

Your guts stat.

*Example:* `base_guts>=1200`

*Meaning:* Your guts stat is 1200 or higher.

## base_power

Your power stat.

*Example:* `base_power>=1200`

*Meaning:* Your power stat is 1200 or higher.

## base_speed

Your speed stat.

*Example:* `base_speed>=1200`

*Meaning:* Your speed stat is 1200 or higher.

## base_stamina

Your stamina stat.

*Example:* `base_stamina>=1200`

*Meaning:* Your Stamina stat is 1200 or higher.

## base_wiz

Your wit stat.

*Example:* `base_wiz>=1200`

*Meaning:* Your wit stat is 1200 or higher.

## bashin_diff_behind

Horse lengths between you and the closest uma behind you.

*Example:* `bashin_diff_behind<=1`

*Meaning:* There's another uma at most one horse length behind you.

## bashin_diff_infront

Horse lengths between you and the closest uma ahead of you.

*Example:* `bashin_diff_infront<=1`

*Meaning:* There's another uma at most one horse length ahead of you.

## behind_near_lane_time

The number of seconds there's another uma right behind you.

*Note:* "Right behind" means no more than 2.5 meters behind, and no more than 1 lane (1/18 of the track's width) to either side. It doesn't have to be the same uma (as long as there's at least one), but if your own placement changes, the timer will reset.

*Example:* `behind_near_lane_time>=3`

*Meaning:* Another uma has been right behind you for at least 3 seconds.

## behind_near_lane_time_set1

The number of seconds there's another uma behind you.

*Note:* This is similar to `behind_near_lane_time` with different values. "Behind" here means no more than 5 meters behind, and no more than 2.7 lanes (2.7/18 of the track's width) to either side. It doesn't have to be the same uma (as long as there's at least one), but if your own placement changes, the timer will reset.

*Example:* `behind_near_lane_time_set1>=1`

*Meaning:* Another uma has been behind you for at least 1 seconds.

## blocked_all_continuetime

The number of seconds you've been blocked from the front and at least one side simultaneously.

*Example:* `blocked_all_continuetime>=1`

*Meaning:* You've been blocked by other umas both in front and on the side for at least one second.

## blocked_front

Checks if you're currently being blocked by someone in front of you.

*Example:* `blocked_front==1`

*Meaning:* Someone is right in front of you.

## blocked_front_continuetime

The number of seconds you've been blocked from the front.

*Example:* `blocked_front_continuetime>=1`

*Meaning:* You've been blocked by another uma in front of you for at least a second.

## blocked_side_continuetime

The number of seconds you've been blocked from at least one side.

*Example:* `blocked_side_continuetime>=2`

*Meaning:* You've been blocked by another uma from either side for at least 2 seconds.

## change_order_onetime

Checks if your position in the race has changed.

*Note:* Positive number means being overtaken, negative number means overtaking someone.

*Example:* `change_order_onetime<0`

*Meaning:* You've overtaken another uma.

## change_order_up_end_after

Checks how many times you've overtaken someone after entering the late-race.

*Example:* `change_order_up_end_after>=3`

*Meaning:* You have overtaken another girl at least 3 times since entering the late-race.

## change_order_up_finalcorner_after

Checks how many times you've overtaken someone after entering the Final Corner.

*Example:* `change_order_up_finalcorner_after>=3`

*Meaning:* You have overtaken another girl at least 3 times since entering the Final Corner.

## change_order_up_middle

Checks how many times you've overtaken someone during the Mid-race.

*Example:* `change_order_up_middle>=3`

*Meaning:* You have overtaken another girl at least 3 times during the Mid-race.

## compete_fight_count

The number of times you've been in a Showdown.

*Note:* Showdown (追い比べ) refers to the mechanic where directly competing with another girl on the Final Straight will give you extra speed and acceleration based on your guts.

*Example:* `compete_fight_count>0`

*Meaning:* You participated in at least one Showdown (追い比べ) during the Final Straight.

## corner

Checks if you're currently on a corner (and if yes, then which one).

*Note:* For example, "first corner" refers to the first corner on the racetrack, not necessarily the first corner you pass through. This condition is often also used as `corner!=0`, meaning any corner.

*Possible values:* 0 (not a corner), 1 (first corner), 2 (second corner), 3 (third corner), 4 (fourth corner)

*Example:* `corner!=0`

*Meaning:* You're currently on any corner.

## corner_count

Checks how many corners the racetrack has.

*Example:* `corner_count==0`

*Meaning:* The racetrack has no corners.

## corner_random

Picks a random point during the selected corner.

*Note:* If the race goes through the same corner multiple times, it will always activate at the last possible opportunity.

*Possible values:* 1 (first corner), 2 (second corner), 3 (third corner), 4 (fourth corner)

*Example:* `corner_random==1`

*Meaning:* A random point on the first corner is selected.

## course_distance

The length of the current race.

*Example:* `course_distance==2400`

*Meaning:* The current race is exactly 2400 meters long.

## distance_diff_rate

Your position between the currently first and the currently last girl as a percentage.

*Example:* `distance_diff_rate<=30`

*Meaning:* If there's a 100-meter gap between the first and the last uma, you're no further than 30 meters behind the first one.

## distance_diff_top

The distance in meters between the first place and you.

*Note:* This condition only uses whole numbers. See `distance_diff_top_float` for the same condition with floating point numbers.

*Example:* `distance_diff_top>=7`

*Meaning:* There's at least 7 meters difference between the character currently in the first place and you.

## distance_diff_top_float

The distance in decimeters between the first place and you.

*Note:* Divide the number by 10 to get the difference in meters. A value of 25 means a difference of 2.5 meters.

*Example:* `distance_diff_top_float<=25`

*Meaning:* There's at most a 2.5 meters difference between the character currently in the first place and you.

## distance_rate

Race progression in percentage.

*Example:* `distance_rate>=50`

*Meaning:* Race is in its half or later.

## distance_rate_after_random

Picks a random point after the specified point in the race.

*Note:* The number is in percentage, for example, 50 meaning 50% of the race.

*Example:* `distance_rate_after_random==50`

*Meaning:* Picks a random point in the second half (i.e. after 50%) of the race.

## distance_type

The distance type of the current race.

*Possible values:* 1 (sprint), 2 (mile), 3 (medium), 4 (long)

*Example:* `distance_type==4`

*Meaning:* The current race is long.

## down_slope_random

Picks a random point on any downhill.

*Example:* `down_slope_random==1`

*Meaning:* A random point on any downhill part of the race is selected.

## fan_count

Checks the number of your fans.

*Example*: `fan_count>=146738`

*Meaning*: You have at least 146738 fans.

## furlong

How many entire furlongs you have completed in the race.

*Example:* `furlong==3`

*Meaning:* You're currently in the fourth furlong (you have completed three furlongs).

## grade

The grade of the current race.

*Note:* All the "big" races without a specified grade (URA Finals, Champions Meeting, Team Trials, etc.) generally fall into the 100 (G1) category.

*Possible values:* 100 (G1), 200 (G2), 300 (G3), 400 (OP), 700 (Pre-OP), 800 (maiden), 900 (debut), 999 (daily)

*Example:* `grade==100`

*Meaning:* The current race is of grade G1.

## ground_condition

The current condition of the track.

*Possible values:* 1 (firm), 2 (good), 3 (soft), 4 (heavy)

*Example:* `ground_condition==1`

*Meaning:* The track condition is firm.

## ground_type

The ground type of the current race.

*Possible values:* 1 (turf), 2 (dirt)

*Example:* `ground_type==2`

*Meaning:* The race is taking place on a dirt track.

## hp_per

Your remaining HP (stamina) in percentage.

*Example:* `hp_per<=70`

*Meaning:* You have at most 70% stamina remaining.

## infront_near_lane_time

The number of seconds there's another uma right in front of you.

*Note:* "Right in front" means no more than 2.5 meters ahead, and no more than 1 lane (1/18 of the track's width) to either side. It doesn't have to be the same uma (as long as there's at least one), but if your own placement changes, the timer will reset.

*Example:* `infront_near_lane_time>=3`

*Meaning:* Another uma has been right in front of you for at least 3 seconds.

## is_abroad

Check whether the race takes place abroad (relative to Japan).

*Example*: `is_abroad==1`

*Meaning*: The race takes place outside of Japan.

## is_activate_any_skill

Checks if any other skill has just been activated.

*Example:* `is_activate_any_skill==1`

*Meaning:* You have just used another skill.

## is_activate_heal_skill

Checks if a recovery (healing) skill has just been activated.

*Example:* `is_activate_heal_skill==1`

*Meaning:* You have just used a recovery skill.

## is_activate_other_skill_detail

A different alternative of this skill was activated during the race.

*Example:* `is_activate_other_skill_detail==1`

*Meaning:* You activated another alternative (e.g. the first one) of this skill at some earlier point of the race.

## is_badstart

Checks if you've had a late start to the current race.

*Note:* Late start" refers to starting gate reaction time of over 0.08 seconds.

*Example:* `is_badstart==0`

*Meaning:* You haven't had a late start to the current race.

## is_basis_distance

The distance type of the track.

*Note:* Core" distance is divisible by 400, "non-core" distance isn't.

*Possible values:* 0 (non-core), 1 (core)

*Example:* `is_basis_distance==1`

*Meaning:* The total distance of the race is divisible by 400 (e.g. 1600, 2000, 2400)

## is_behind_in

Checks if the girl behind you is closer to the inner fence than you.

*Example:* `is_behind_in==1`

*Meaning:* The uma behind you is closer to the inner fence than you.

## is_dirtgrade

The race is an exchange race (takes place at either the Kawasaki, Funabashi, Morioka, or Ooi racetrack)

*Note:* More generally, any race held on a racetrack which is considered to be a 「地方」 (local) racetrack (as opposed to the bigger 「中央」 (Center) racetracks) will count for this.

*Possible values:* 0 (False), 1 (True)

*Example:* `is_dirtgrade==1`

*Meaning:* The current race is an exchange race (takes place at either the Kawasaki, Funabashi, Morioka, or Ooi racetrack)

## is_exist_chara_id

Checks if a character with the given ID is participating in the race.

*Note:* Only checks for the character, not the version of her. If a seasonal version of an uma exists, it would fulfill the condition too.

*Example:* `is_exist_chara_id==1002`

*Meaning:* Silence Suzuka is running in the current race.

## is_exist_skill_id

Checks if a character with the given skill ID is participating in the race.

*Example:* `is_exist_skill_id==201631`

*Meaning:* Someone with the "Sympathy" skill is running.

## is_finalcorner

Checks if you're currently on the final corner or beyond.

*Note:* "Beyond" means that this can activate on the final straight if there's one after the final corner. However, if there are no corners in the race, this will never activate.

*Example:* `is_finalcorner==1`

*Meaning:* You're currently on the last corner or beyond it.

## is_finalcorner_laterhalf

Checks if you're currently on the second half of the final corner.

*Example:* `is_finalcorner_laterhalf==1`

*Meaning:* You're on the second half of the final corner.

## is_finalcorner_random

Picks a random point on the final corner. The skill will activate there if all other conditions are met.

*Note:* Unlike `is_finalcorner`, this condition refers to the actual last corner only (can't pick a point on the last straight).

*Example:* `is_finalcorner_random==1`

*Meaning:* A random point on the final corner.

## is_goodstart

Checks if you've had a good start to the current race.

*Note:* "Good start" refers to starting gate reaction time of under 0.02 seconds. This definition is under investigation.

*Example:* `is_goodstart==1`

*Meaning:* You've had a good start to the current race.

## is_hp_empty_onetime

Checks if your HP (stamina) had been depleted at some point in the race.

*Example:* `is_hp_empty_onetime==1`

*Meaning:* You're out of stamina, or had been previously.

## is_last_straight

Checks if you're currently on the final straight of the course.

*Note:* If a course only has one straight, the first straight is the last straight.

*Example:* `is_last_straight==1`

*Meaning:* You're currently on the final straight of the course.

## is_last_straight_onetime

Checks if you've just entered the last straight.

*Example:* `is_last_straight_onetime==1`

*Meaning:* You have just arrived at the last straight.

## is_lastspurt

Checks if you're currently performing your last spurt.

*Example:* `is_lastspurt==1`

*Meaning:* You're in the last spurt mode.

## is_move_lane

Checks if you've just moved left or right.

*Possible values:* 1 (moved closer to the fence), 2 (moved further from the fence)

*Example:* `is_move_lane==1`

*Meaning:* You've just moved closer to the inner fence.

## is_other_character_activate_advantage_skill

A character other than you has activated an advantageous (i.e. non-debuff) skill of a specific type.

*Example:* `is_other_character_activate_advantage_skill==9`

*Meaning:* Someone has activated a healing skill.

## is_overtake

Checks if you have any overtake targets.

*Note:* A girl is an overtake target if she's up to 20 meters ahead of you, and you can catch up with her within 15 seconds at the current speed.

*Example:* `is_overtake==1`

*Meaning:* You have at least one overtake target.

## is_popularity_top_character_activate_advantage_skill

The most popular character (possibly you) has activated an advantageous (i.e. non-debuff) skill of a specific type, or any skill if the value is -1.

*Example:* `is_popularity_top_character_activate_advantage_skill==-1`

*Meaning:* The most popular uma has just activated a skill.

## is_surrounded

Checks if you're currently surrounded.

*Note:* You're being surrounded if there are other uma musume that fulfill the following conditions:

— No more than 3 meters ahead of you, and less than 1.5 lanes difference between you
— No more than 3 meters behind you, and less than 1.5 lanes difference between you
— Less than 1.5 meters behind or ahead of you, and less than 3 lanes difference between you, but not the same lane

In simpler terms, this will usually mean that there's an uma in front of you, behind you, and next to you. It's technically possible for one girl to fulfill two of these conditions (someone passing you from the adjacent line), so two umas can still surround you.

*Example:* `is_surrounded==1`

*Meaning:* You're currently surrounded by other runners.

## is_temptation

Checks if you're currently rushing.

*Example:* `is_temptation==0`

*Meaning:* You're currently not rushing.

## is_tight_track

Check whether the racetrack has tight corners.

*Note:* So far, this applies to Sapporo, Hakodate, Fukushima, Kokura, Kawasaki, and Funabashi.

*Example*: `is_tight_track==1`

*Meaning*: The racetrack has tight corners.

## is_used_skill_id

Checks whether you've activated a specific skill during this race.

*Example:* `is_used_skill_id==202051`

*Meaning:* You've activated the "Runaway" (大逃げ) skill during this race.

## is_used_skill_id_with_detail_one

Checks whether you've activated a specific skill during this race through its first alternative (see the note at the top about skills with multiple alternatives).

*Example:* `is_used_skill_id_with_detail_one==204452`

*Meaning:* You've activated the "聖女の試練" skill during this race, and it applied its first alternative's effect (keeping your place to raise your speed), not its second one (lowering your speed).

## lane_type

The lane you're currently running in.

*Possible values:* depending on your distance from the inner fence, in course widths (1 course width = 11.25 meters):

- 0 (inner): up to 0.2 (2.25 meters).
- 1 (middle): over 0.2, up to 0.4 (4.5 meters).
- 2 (outer): over 0.4, up to 0.6 (6.75 meters).
- 3 (outside): over 0.6.

*Example:* `lane_type==0`

*Meaning:* You're currently running right next to the inner fence.

## last_straight_random

Picks a random point on the last straight.

*Example:* `last_straight_random==1`

*Meaning:* A random point on the last straight is selected.

## lastspurt

Checks if you can carry out your last spurt.

*Possible values:* 0 (not enough stamina to even finish at base speed), 1 (enough stamina to run above base speed, but not enough to finish at max speed), 2 (enough stamina to finish at max speed)

*Example:* `lastspurt==2`

*Meaning:* You have more than enough stamina (HP) left to carry out your last spurt at full strength.

## motivation

Your mood going into the race.

*Possible values:* 1 (Awful, 絶不調), 2 (Bad, 不調), 3 (Normal, 普通), 4 (Good, 好調), 5 (Great, 絶好調)

*Example:* `motivation>=4`

*Meaning:* Your mood is Good or better.

## near_count

The number of other runners that are currently near you.

*Note:* "Near" means no further than 3 meters behind/ahead, and no further than 3 lanes to either side. 1 lane equals to 1/18 of the track's width.

*Example:* `near_count==4`

*Meaning:* There are exactly four other runners near you.

## near_infront_count

The number of other runners right in front of you at the moment.

*Note:* "Right in front" means no more than 2.5 meters ahead.

*Example:* `near_infront_count==0`

*Meaning:* There's nobody right in front of you.

## order

Your current position in the race.

*Example:* `order<=3`

*Meaning:* You're currently first, second, or third.

## order_rate

Your current position in the race as a percentage.

*Note:* The order rate condition is converted to an order condition, rounding to the nearest. For example, consider `order_rate>50` in a 9-uma race: 9×50% = 4.5, which rounds to 5. The condition is then converted to `order>5`, and can only trigger when you're 6th or worse, not 5th or worse as you might expect.

*Example:* `order_rate>=50`

*Meaning:* You're in the second half of the pack.

## order_rate_in20_continue

Your position has been within the top 20% for the entire race until now.

*Note:* The first 5 seconds of the race don't count, so you have time to establish your position. Same as with normal `order_rate`, the percentage is converted to an order, rounding to the nearest. It's equivalent to `order_rate<=20` holding for the entire race.

*Example:* `order_rate_in20_continue==1`

*Meaning:* Your position in the race was never worse than top 20% after the first 5 seconds. For example, in a Champions Meeting (9-uma race), you were never worse than 2nd; in a League of Heroes (12-uma race), you were never worse than 2nd.

## order_rate_in40_continue

Your position has been within the top 40% for the entire race until now.

*Note:* The first 5 seconds of the race don't count, so you have time to establish your position. Same as with normal `order_rate`, the percentage is converted to an order, rounding to the nearest. It's equivalent to `order_rate<=40` holding for the entire race.

*Example:* `order_rate_in40_continue==1`

*Meaning:* Your position in the race was never worse than top 40% after the first 5 seconds. For example, in a Champions Meeting (9-uma race), you were never worse than 4th; in a League of Heroes (12-uma race), you were never worse than 5th.

## order_rate_in50_continue

Your position has been within the top 50% for the entire race until now.

*Note:* The first 5 seconds of the race don't count, so you have time to establish your position. Same as with normal `order_rate`, the percentage is converted to an order, rounding to the nearest. It's equivalent to `order_rate<=50` holding for the entire race.

*Example:* `order_rate_in50_continue==1`

*Meaning:* Your position in the race was never worse than top 50% after the first 5 seconds. For example, in a Champions Meeting (9-uma race), you were never worse than 5th; in a League of Heroes (12-uma race), you were never worse than 6th.

## order_rate_in80_continue

Your position has been within the top 80% for the entire race until now.

*Note:* The first 5 seconds of the race don't count, so you have time to establish your position. Same as with normal `order_rate`, the percentage is converted to an order, rounding to the nearest. It's equivalent to `order_rate<=80` holding for the entire race.

*Example:* `order_rate_in80_continue==1`

*Meaning:* Your position in the race was never worse than top 80% after the first 5 seconds. For example, in a Champions Meeting (9-uma race), you were never worse than 7th; in a League of Heroes (12-uma race), you were never worse than 10th.

## order_rate_out20_continue

Your position has been worse than the top 20% for the entire race until now.

*Note:* The first 5 seconds of the race don't count, so you have time to establish your position. Same as with normal `order_rate`, the percentage is converted to an order, rounding to the nearest. It's equivalent to `order_rate>=20` holding for the entire race, so the rounded order itself counts as outside of the top 20%.

*Example:* `order_rate_out20_continue==1`

*Meaning:* Your position in the race was always outside of the top 20% (so within the last 80%). For example, in a Champions Meeting (9-uma race), you were always 2nd or worse; in a League of Heroes (12-uma race), you were always 2nd or worse.

## order_rate_out40_continue

Your position has been worse than the top 40% for the entire race until now.

*Note:* The first 5 seconds of the race don't count, so you have time to establish your position. Same as with normal `order_rate`, the percentage is converted to an order, rounding to the nearest. It's equivalent to `order_rate>=40` holding for the entire race, so the rounded order itself counts as outside of the top 40%.

*Example:* `order_rate_out40_continue==1`

*Meaning:* Your position in the race was always outside of the top 40% (so within the last 60%). For example, in a Champions Meeting (9-uma race), you were always 4th or worse; in a League of Heroes (12-uma race), you were always 5th or worse.

## order_rate_out50_continue

Your position has been worse than the top 50% for the entire race until now.

*Note:* The first 5 seconds of the race don't count, so you have time to establish your position. Same as with normal `order_rate`, the percentage is converted to an order, rounding to the nearest. It's equivalent to `order_rate>=50` holding for the entire race, so the rounded order itself counts as outside of the top 50%.

*Example:* `order_rate_out50_continue==1`

*Meaning:* Your position in the race was always outside of the top 50% (so within the last 50%). For example, in a Champions Meeting (9-uma race), you were always 5th or worse; in a League of Heroes (12-uma race), you were always 6th or worse.

## order_rate_out70_continue

Your position has been worse than the top 70% for the entire race until now.

*Note:* The first 5 seconds of the race don't count, so you have time to establish your position. Same as with normal `order_rate`, the percentage is converted to an order, rounding to the nearest. It's equivalent to `order_rate>=70` holding for the entire race, so the rounded order itself counts as outside of the top 70%.

*Example:* `order_rate_out70_continue==1`

*Meaning:* Your position in the race was always outside of the top 70% (so within the last 30%). For example, in a Champions Meeting (9-uma race), you were always 6th or worse; in a League of Heroes (12-uma race), you were always 8th or worse.

## overtake_target_no_order_up_time

The number of seconds you've had any overtake targets.

*Note:* A girl is an overtake target if she's up to 20 meters ahead of you, and if you can catch up with her within 15 seconds at the current speed.

*Example:* `overtake_target_no_order_up_time>=2`

*Meaning:* You've had at least one overtake target for at least two seconds.

## overtake_target_time

The number of seconds you've been an overtake target.

*Note:* You're an overtake target of another girl if you're up to 20 meters ahead of her, and if she can catch up with you within 15 seconds at the current speed.

*Example:* `overtake_target_time>=2`

*Meaning:* You've been an overtake target for at least 2 seconds.

## phase

Current phase of the race.

*Possible values:* 0 (Early-race), 1 (Mid-race), 2 (Late-race), 3 (Last leg)

*Example:* `phase>=2`

*Meaning:* You're currently either in the late-race or the last leg.

## phase_corner_random

Picks a random point during a corner on the selected phase. The skill will activate there if all other conditions are met.

*Possible values:* 0 (Early-race), 1 (Mid-race), 2 (Late-race), 3 (Last leg)

*Example:* `phase_corner_random==1`

*Meaning:* A random point during a corner in the Mid-race.

## phase_first_half_straight_random

Picks a random point on any straight in the first half of the given phase.

*Example:* `phase_first_half_straight_random==1`

*Meaning:* A random point on any straight in the first half of the middle leg is selected.

## phase_firsthalf

Checks if you're in the first half of the selected phase.

*Possible values:* 0 (Early-race), 1 (Mid-race), 2 (Late-race), 3 (Last leg)

*Example:* `phase_firsthalf==1`

*Meaning:* You're anywhere in the first half of the middle leg.

## phase_firsthalf_random

Picks a random point during the first half of the selected phase.

*Possible values:* 0 (Early-race), 1 (Mid-race), 2 (Late-race), 3 (Last leg)

*Example:* `phase_firsthalf_random==1`

*Meaning:* A random point on the first half of the Mid-race is selected.

## phase_firstquarter

Checks if you're in the first quarter of the selected phase.

*Possible values:* 0 (Early-race), 1 (Mid-race), 2 (Late-race), 3 (Last leg)

*Example:* `phase_firstquarter==1`

*Meaning:* You're anywhere in the first quarter of the middle leg.

## phase_firstquarter_random

Picks a random point during the first quarter of the selected phase.

*Possible values:* 0 (Early-race), 1 (Mid-race), 2 (Late-race), 3 (Last leg)

*Example:* `phase_firstquarter_random==1`

*Meaning:* A random point on the first quarter of the Mid-race is selected.

## phase_laterhalf

Checks if you're in the second half of the selected phase.

*Possible values:* 0 (Early-race), 1 (Mid-race), 2 (Late-race), 3 (Last leg)

*Example:* `phase_laterhalf==1`

*Meaning:* You're anywhere in the second half of the middle leg.

## phase_laterhalf_random

Picks a random point during the second half of the selected phase. The skill will activate there if all other conditions are met.

*Possible values:* 0 (Early-race), 1 (Mid-race), 2 (Late-race), 3 (Last leg)

*Example:* `phase_laterhalf_random==0`

*Meaning:* A random point in the second half of the Early-race.

## phase_latter_half_straight_random

Picks a random point on any straight in the second half of the given phase.

*Example:* `phase_latter_half_straight_random==1`

*Meaning:* A random point on any straight in the second half of the middle leg is selected.

## phase_random

Picks a random point during the selected phase. The skill will activate there if all other conditions are met.

*Possible values:* 0 (Early-race), 1 (Mid-race), 2 (Late-race), 3 (Last leg)

*Example:* `phase_random==0`

*Meaning:* A random point in the Early-race.

## phase_straight_random

Picks a random point on any straight in the given phase.

*Example:* `phase_straight_random==1`

*Meaning:* A random point on any straight in the middle leg is selected.

## popularity

Your popularity rank among the runners in the current race.

*Example:* `popularity==1`

*Meaning:* You're the most popular girl in the race.

## post_number

The starting gate block you started to race in.

*Note:* Gate block is not the same as the gate number. In races with a lot of participants, one block can have several gates.

*Example:* `post_number<=3`

*Meaning:* You started the race in starting gate blocks 1, 2, or 3.

## random_lot

Rolls a random number from 0-100.

*Note:* This condition is used as `random_lot==number`, where number is the chance of winning the roll in percentage.

*Example:* `random_lot==50`

*Meaning:* 50% chance for activation.

## random_lot_shared

Rolls a random number from 0-100, once per skill.

*Note:* Same as random_lot, but only one number is rolled and shared by all skill's `random_lot_shared` conditions.

*Example:* `random_lot_shared==50`

*Meaning:* 50% chance for activation. If multiple conditions have `random_lot_shared`, they will all use the same roll.

## remain_distance

The number of meters remaining in the race.

*Example:* `remain_distance==200`

*Meaning:* There are exactly 200 meters remaining in the race.

## remain_distance_viewer_id

The number of meters remaining in the race for any player character.

*Example:* `remain_distance_viewer_id<=850`

*Meaning:* Any non-NPC character has reached the last 850 meters in the race.

## rotation

The direction of the race.

*Possible values:* 1 (clockwise/right), 2 (counterclockwise/left)

*Example:* `rotation==1`

*Meaning:* The current race is run clockwise.

## run_at_full_speed_random

Picks a random point during your all-out spurt (全開スパート).

*Example:* `run_at_full_speed_random==1`

*Meaning:* A random point during your all-out spurt is selected.

## running_style

Your running strategy.

*Possible values:* 1 (front runner), 2 (pace chaser), 3 (late surger), 4 (end closer)

*Example:* `running_style==1`

*Meaning:* You're a runner.

## running_style_count_nige_otherself

The number of front runners in the race.

*Note:* You're not included in this number.

*Example:* `running_style_count_nige_otherself>=1`

*Meaning:* At least one of the other runners in the race is a front runner.

## running_style_count_oikomi_otherself

The number of end closers in the race.

*Note:* You're not included in this number.

*Example:* `running_style_count_oikomi_otherself>=1`

*Meaning:* At least one of the other runners in the race is an end closer.

## running_style_count_same

The number of characters with the same running strategy as you.

*Note:* You count as well.

*Example:* `running_style_count_same<=1`

*Meaning:* Nobody shares the same strategy with you (e.g. if you're a front runner, you're the only front runner).

## running_style_count_same_rate

The percentage of characters with the same running strategy as you.

*Note:* You count as well.

*Example:* `running_style_count_same_rate>=40`

*Meaning:* At least 40% of the runners in the current race have the same strategy as you (e.g. front runner).

## running_style_count_sashi_otherself

The number of late surgers in the race.

*Note:* You're not included in this number.

*Example:* `running_style_count_sashi_otherself>=1`

*Meaning:* At least one of the other runners in the race is a late surgers.

## running_style_count_senko_otherself

The number of pace chasers in the race.

*Note:* You're not included in this number.

*Example:* `running_style_count_senko_otherself>=1`

*Meaning:* At least one of the other runners in the race is a pace chaser.

## running_style_equal_popularity_one

Checks if your running strategy is the same as the strategy of the most popular girl in the race.

*Example:* `running_style_equal_popularity_one==1`

*Meaning:* You share the same running strategy with the #1 popular girl in the race.

## running_style_temptation_count_nige

The number of front runners that are currently rushing.

*Note:* You count as well.

*Example:* `running_style_temptation_count_nige>=1`

*Meaning:* There's at least one currently rushing front runner in the race.

## running_style_temptation_count_oikomi

The number of end closers that are currently rushing.

*Note:* You count as well.

*Example:* `running_style_temptation_count_oikomi>=1`

*Meaning:* There's at least one currently rushing end closer in the race.

## running_style_temptation_count_sashi

The number of late surgers that are currently rushing.

*Note:* You count as well.

*Example:* `running_style_temptation_count_sashi>=1`

*Meaning:* There's at least one currently rushing late surger in the race.

## running_style_temptation_count_senko

The number of pace chasers that are currently rushing.

*Note:* You count as well.

*Example:* `running_style_temptation_count_senko>=1`

*Meaning:* There's at least one currently rushing pace chaser in the race.

## running_style_temptation_opponent_count_nige

The number of enemy front runners that are currently rushing.

*Example:* `running_style_temptation_opponent_count_nige>=1`

*Meaning:* There's at least one currently rushing enemy front runner in the race.

## running_style_temptation_opponent_count_oikomi

The number of enemy end closers that are currently rushing.

*Example:* `running_style_temptation_opponent_count_oikomi>=1`

*Meaning:* There's at least one currently rushing enemy end closer in the race.

## running_style_temptation_opponent_count_sashi

The number of enemy late surgers that are currently rushing.

*Example:* `running_style_temptation_opponent_count_sashi>=1`

*Meaning:* There's at least one currently rushing enemy late surger in the race.

## running_style_temptation_opponent_count_senko

The number of enemy pace chasers that are currently rushing.

*Example:* `running_style_temptation_opponent_count_senko>=1`

*Meaning:* There's at least one currently rushing pace chaser in the race.

## same_skill_horse_count

The number of runners that have this particular skill.

*Note:* You count as well.

*Example:* `same_skill_horse_count==1`

*Meaning:* Among all the umas in the race, you're the only one with this particular skill.

## season

The current season.

*Note:* The "Cherry blossom" season refers to late spring, when cherry trees around the race track visibly bloom. Skills that are supposed to activate in spring will generally have "1 or 5" in their season conditions.

*Possible values:* 1 (spring), 2 (summer), 3 (fall), 4 (winter), 5 (cherry blossom)

*Example:* `season==1@season==5`

*Meaning:* It's currently spring (either early spring or the cherry blossom season).

## slope

Checks if you're currently running uphill or downhill.

*Possible values:* 0 (no slope), 1 (uphill), 2 (downhill)

*Example:* `slope==1`

*Meaning:* You're currently running uphill.

## straight_front_type

Checks if you're currently on a specific straight.

*Possible values:* 1 (straight in front of the audience), 2 (straight on the opposite side from the audience)

*Example:* `straight_front_type==2`

*Meaning:* You're currently on a straight on the opposite side of the racecourse from where the audience is.

## straight_random

Picks a random point on any straight.

*Note:* First rolls for a straight segment, and then for a random point on that segment. Thus, if there's one very short straight and one very long straight, they both have the same chance to be selected. "Straight" generally only means the straight part of the track next to the stadium seatings, or the one across from there. In other words, tracks can have parts that are neither a straight nor a corner.

*Example:* `straight_random==1`

*Meaning:* A random point on any straight is selected.

## succession_skill_count

The number of inherited unique skills you have.

*Example:* `succession_skill_count>=3`

*Meaning:* You have at least 3 inherited unique skills.

## temptation_count

The number of times you've been rushing during the race.

*Example:* `temptation_count==0`

*Meaning:* You haven't rushed a single time during the race.

## temptation_count_behind

The number of runners behind you that are currently rushing.

*Example:* `temptation_count_behind>=1`

*Meaning:* At least one uma behind you is rushing.

## temptation_count_infront

The number of runners in front of you that are currently rushing.

*Example:* `temptation_count_infront>=1`

*Meaning:* At least one uma ahead of you is rushing.

## temptation_opponent_count_behind

The number of enemies behind you that are currently rushing.

*Example:* `temptation_opponent_count_behind>=1`

*Meaning:* At least one enemy behind you is rushing.

## temptation_opponent_count_infront

The number of enemies in front of you that are currently rushing.

*Example:* `temptation_opponent_count_infront>=1`

*Meaning:* At least one enemy ahead of you is rushing.

## time

The time of day during which the race takes place.

*Note:* This refers to the icon displayed alongside race details.

*Possible values:* 0 (Any), 1 (Morning), 2 (Daytime), 3 (Evening), 4 (Night)

*Example:* `time==4`

*Meaning:* It's currently night.

## track_id

The ID of the current racetrack.

*Some possible values:* 10001 (Sapporo, 札幌), 10002 (Hakodate, 函館), 10003 (Niigata, 新潟), 10004 (Fukushima, 福島), 10005 (Nakayama, 中山), 10006 (Tokyo, 東京), 10007 (Chukyo, 中京), 10008 (Kyoto, 京都), 10009 (Hanshin, 阪神), 10010 (Kokura, 小倉), 10101 (Ooi, 大井), 10103 (Kawasaki, 川崎), 10104 (Funabashi, 船橋), 10105 (Morioka, 盛岡)

*Example:* `track_id==10006`

*Meaning:* The race is taking place on the Tokyo racetrack.

## up_slope_random

Picks a random point on any uphill.

*Example:* `up_slope_random==1`

*Meaning:* A random point on any uphill part of the race is selected.

## up_slope_random_later_half

Picks a random point on any uphill in the second half of the race.

*Example:* `up_slope_random_later_half==1`

*Meaning:* A random point on any uphill in the second half of race is selected.

## visiblehorse

The number of runners in your field of vision.

*Example:* `visiblehorse>=4`

*Meaning:* There are at least 4 runners in your field of vision.

## weather

The current weather.

*Possible values:* 1 (sunny), 2 (cloudy), 3 (rainy), 4 (snowy)

*Example:* `weather==4`

*Meaning:* It's snowing.
