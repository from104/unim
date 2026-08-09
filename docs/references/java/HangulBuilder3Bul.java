package com.seojin.hangul;

import java.util.HashMap;
import java.util.Stack;


public class HangulBuilder3Bul extends HangulBuilder {

	public HangulBuilder3Bul() {
		super();
		
		combinedJamo.put(Cho.G, new HashMap<Object, Object>());
		combinedJamo.get(Cho.G).put(Cho.G, Cho.GG);
		
		combinedJamo.put(Cho.D, new HashMap<Object, Object>());
		combinedJamo.get(Cho.D).put(Cho.D, Cho.DD);
		
		combinedJamo.put(Cho.B, new HashMap<Object, Object>());
		combinedJamo.get(Cho.B).put(Cho.B, Cho.BB);
		
		combinedJamo.put(Cho.S, new HashMap<Object, Object>());
		combinedJamo.get(Cho.S).put(Cho.S, Cho.SS);
		
		combinedJamo.put(Cho.J, new HashMap<Object, Object>());
		combinedJamo.get(Cho.J).put(Cho.J, Cho.JJ);
		
		combinedJamo.put(Jung.O, new HashMap<Object, Object>());
		combinedJamo.get(Jung.O).put(Jung.A, Jung.WA);
		combinedJamo.get(Jung.O).put(Jung.AE, Jung.WAE);
		combinedJamo.get(Jung.O).put(Jung.I, Jung.OE);

		combinedJamo.put(Jung.U, new HashMap<Object, Object>());
		combinedJamo.get(Jung.U).put(Jung.EO, Jung.WEO);
		combinedJamo.get(Jung.U).put(Jung.E, Jung.WE);
		combinedJamo.get(Jung.U).put(Jung.I, Jung.WI);
		
		combinedJamo.put(Jung.EU, new HashMap<Object, Object>());
		combinedJamo.get(Jung.EU).put(Jung.I, Jung.YI);
		
		combinedJamo.put(Jong.G, new HashMap<Object, Object>());
		combinedJamo.get(Jong.G).put(Jong.G, Jong.GG);
		combinedJamo.get(Jong.G).put(Jong.S, Jong.GS);
		
		combinedJamo.put(Jong.N, new HashMap<Object, Object>());
		combinedJamo.get(Jong.N).put(Jong.J, Jong.NJ);
		combinedJamo.get(Jong.N).put(Jong.H, Jong.NH);
		
		combinedJamo.put(Jong.L, new HashMap<Object, Object>());
		combinedJamo.get(Jong.L).put(Jong.G, Jong.LG);
		combinedJamo.get(Jong.L).put(Jong.M, Jong.LM);
		combinedJamo.get(Jong.L).put(Jong.B, Jong.LB);
		combinedJamo.get(Jong.L).put(Jong.S, Jong.LS);
		combinedJamo.get(Jong.L).put(Jong.T, Jong.LT);
		combinedJamo.get(Jong.L).put(Jong.P, Jong.LP);
		combinedJamo.get(Jong.L).put(Jong.H, Jong.LH);
		
		combinedJamo.put(Jong.B, new HashMap<Object, Object>());
		combinedJamo.get(Jong.B).put(Jong.S, Jong.BS);
		
		combinedJamo.put(Jong.S, new HashMap<Object, Object>());
		combinedJamo.get(Jong.S).put(Jong.S, Jong.SS);
	}

	@SuppressWarnings("unchecked")
	@Override
	public HangulChar addJamo(Object jamo) {
		if (isJamo(jamo)) {
			mJamoStack.push(jamo);
			if (buildHangul()) {
				return null;
			} else {
				mJamoStack.pop();
				buildHangul();
				HangulChar completeHangul = new HangulChar(getCho(),getJung(),getJong());
				mLastJamoStack = (Stack<Object>) mJamoStack.clone();
				mJamoStack.clear();
				mJamoStack.push(jamo);
				
				clear();
				buildHangul();
				
				return completeHangul;
			}
		} else {
			return null;
		}
	}

	@Override
	public boolean buildHangul () {
		Object lastPrevJamo = null, lastJamo;

		if (mJamoStack.empty()) { // 스택이 비어있으면
			clear();
			return true;
		} else
			lastJamo = mJamoStack.peek();

		if (mJamoStack.size() > 1) {
			lastPrevJamo = mJamoStack.get(mJamoStack.size()-2);
		}
		
		if ((isFilledOnlyCho() || isFilledOnlyJung()) && isJong(lastJamo))
			return false;
		else if ((isJung(lastPrevJamo) || isJong(lastPrevJamo)) && isCho(lastJamo))
			return false;
		else if (isJong(lastPrevJamo) && isJung(lastJamo))
			return false;
		
		if (!buildCho())
			return false;
		if (!buildJung())
			return false;
		if (!buildJong())
			return false;
		
		return true;
	}
}
